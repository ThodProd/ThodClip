using System.Diagnostics;
using System.Reflection;
using System.Security.Cryptography;
using System.Text.Json;
using System.Windows.Forms;
using ThodClip.Core;

namespace ThodClip.Worker;

internal sealed class Engine : IAsyncDisposable
{
    private const long MaxInlineBytes = 5_500_000;
    private readonly string _root;
    private readonly ClipSettings _settings;
    private readonly ClipStore _store;
    private readonly ClipboardWatcher _clipboard;
    private readonly ScreenshotWatcher _screenshots;
    private readonly CaptureLease _captureLease;
    private readonly CancellationTokenSource _lifetime = new();
    private readonly SemaphoreSlim _importGate = new(1, 1);
    private readonly object _stateGate = new();
    private readonly bool _background;
    private Task? _importTask;
    private ImportSummary? _lastImport;
    private string? _lastImportError;
    private volatile bool _importRunning;
    private volatile bool _ownsCapture;
    private long _lastCaptureMs;
    private long _lastShotMs;
    private readonly string _startedAt = DateTimeOffset.UtcNow.ToString("O");

    public Engine(string dataDir, bool background = false)
    {
        _root = Path.GetFullPath(dataDir);
        _background = background;
        _settings = SettingsStore.Load(_root);
        _store = new ClipStore(_root, _settings);
        _captureLease = new CaptureLease(InstanceNames.CaptureMutex(_root));
        _clipboard = new ClipboardWatcher(_settings.HonorClipboardExclusions) { CaptureEnabled = false };
        _screenshots = new ScreenshotWatcher(_store) { CaptureEnabled = false };
        _clipboard.Captured += OnCaptured;
        _screenshots.Imported += _ => _lastShotMs = Now();
    }

    public void Start()
    {
        _clipboard.Start();
        _screenshots.Start();
        _ = Task.Run(MaintenanceAsync);
        _ = Task.Run(CaptureLeaseLoopAsync);
        Log.Info($"started: data={_root}, background={_background}, clips={_store.GetStats().ClipCount}");
    }

    private async Task CaptureLeaseLoopAsync()
    {
        while (!_lifetime.IsCancellationRequested)
        {
            if (!_ownsCapture && _captureLease.TryAcquire())
            {
                _ownsCapture = true;
                ApplyCaptureGating();
                Log.Info(_background ? "background capture acquired" : "in-hub capture acquired");
                if (_settings.AutoImportScreenshots) StartImport();
            }
            try { await Task.Delay(1500, _lifetime.Token); }
            catch (OperationCanceledException) { break; }
        }
    }

    private async Task MaintenanceAsync()
    {
        try
        {
            _store.VerifyShots();
            var upgraded = _store.UpgradeImageFileClips();
            if (upgraded > 0) Log.Info($"converted {upgraded} copied picture entries to previews");
            var references = _store.UpgradeImageReferenceClips();
            if (references > 0) Log.Info($"resolved {references} picture-name entries to previews");
            ArchivePendingClipboardScreenshots();
        }
        catch (Exception ex)
        {
            Log.Warn("history maintenance failed: " + ex.Message);
        }

        if (!_background && _settings.RunInBackground)
        {
            try
            {
                Autostart.Apply(true, _root);
                Log.Info("background mode verified");
            }
            catch (Exception ex)
            {
                Log.Warn("cannot verify background mode: " + ex.Message);
            }
        }
    }

    /// <summary>
    /// Older builds stored Snipping Tool captures only in the clipboard history. Recover them once
    /// the screenshot detector knows about ScreenClip temporary files.
    /// </summary>
    private void ArchivePendingClipboardScreenshots()
    {
        var page = _store.QueryClips(null, "image", false, null, 0, 200);
        var before = _store.GetStats().ShotCount;
        foreach (var clip in page.Items)
        {
            if (clip.Files.Count == 0) continue;
            if (!clip.Files.Any(file => ScreenshotScanner.IsScreenshotPath(file))) continue;
            try
            {
                var path = _store.ResolveBlob(clip);
                if (!File.Exists(path)) continue;

                // Hash the blob without loading it first: on later starts every recovered image is
                // already in the archive and should not be read into memory again.
                string hash;
                using (var stream = File.OpenRead(path))
                    hash = Convert.ToHexString(SHA256.HashData(stream)).ToLowerInvariant();
                if (_store.ShotHashExists(hash)) continue;

                var source = string.IsNullOrWhiteSpace(clip.Source) ? "Буфер обмена" : "Буфер обмена · " + clip.Source;
                _store.AddScreenshotPng(File.ReadAllBytes(path), source, $"Снимок {clip.Width}×{clip.Height}");
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidDataException)
            {
                Log.Warn("cannot recover clipboard screenshot: " + ex.Message);
            }
        }
        var added = _store.GetStats().ShotCount - before;
        if (added > 0) Log.Info($"archived {added} earlier clipboard screenshots");
    }

    private void ApplyCaptureGating()
    {
        var capture = _ownsCapture && _settings.CaptureEnabled;
        _clipboard.CaptureEnabled = capture;
        _screenshots.CaptureEnabled = capture;
    }

    private static long Now() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

    private static string Iso(long milliseconds) =>
        DateTimeOffset.FromUnixTimeMilliseconds(milliseconds).ToString("O");

    private void OnCaptured(ClipboardPayload payload)
    {
        if (!_settings.CaptureEnabled) return;
        try
        {
            long id = payload.Kind switch
            {
                "text" => _store.AddText(payload.Text, payload.Source),
                "files" => _store.AddFiles(payload.Files, payload.Source),
                "image" when payload.Png is not null => _store.AddImage(payload.Png, payload.Width, payload.Height,
                    payload.Source, payload.Files),
                _ => 0
            };
            if (id > 0)
            {
                _lastCaptureMs = Now();
                Log.Info($"captured {payload.Kind} ({payload.Png?.Length ?? payload.Text.Length} bytes) from {payload.Source ?? "unknown"}");
            }
            if (payload.Kind == "image" && payload.Png is not null && LooksLikeScreenshot(payload))
            {
                var source = string.IsNullOrWhiteSpace(payload.Source) ? "Буфер обмена" : "Буфер обмена · " + payload.Source;
                var shot = _store.AddScreenshotPng(payload.Png, source, $"Снимок {payload.Width}×{payload.Height}");
                if (!string.IsNullOrEmpty(shot)) _lastShotMs = Now();
            }
        }
        catch (Exception ex)
        {
            Log.Error("cannot store clipboard entry", ex);
        }
    }

    private static bool LooksLikeScreenshot(ClipboardPayload payload)
    {
        var archiveAll = Environment.GetEnvironmentVariable("THODCLIP_ARCHIVE_ALL_IMAGES") == "1";
        try
        {
            var screen = Screen.PrimaryScreen?.Bounds ?? Rectangle.Empty;
            return ScreenshotDetector.IsLikelyScreenshot(payload.Files, payload.Source, payload.Width, payload.Height,
                screen.Width, screen.Height, archiveAll);
        }
        catch (Exception ex) when (ex is InvalidOperationException or System.ComponentModel.Win32Exception)
        {
            return ScreenshotDetector.IsLikelyScreenshot(payload.Files, payload.Source, payload.Width, payload.Height,
                0, 0, archiveAll);
        }
    }

    private bool StartImport(bool force = false)
    {
        if (!force && !_ownsCapture) return false;
        lock (_stateGate)
        {
            if (_importTask is { IsCompleted: false }) return false;
            _importRunning = true;
            _lastImportError = null;
            _importTask = Task.Run(async () =>
            {
                await _importGate.WaitAsync(_lifetime.Token);
                try
                {
                    Log.Info("importing existing screenshots");
                    var summary = ScreenshotScanner.Import(_store, _lifetime.Token);
                    lock (_stateGate)
                    {
                        _lastImport = summary;
                        if (summary.Imported > 0) _lastShotMs = Now();
                    }
                    Log.Info($"screenshot import: scanned={summary.Scanned}, imported={summary.Imported}, failed={summary.Failed}");
                }
                catch (OperationCanceledException) { }
                catch (Exception ex)
                {
                    lock (_stateGate) _lastImportError = ex.Message;
                    Log.Error("screenshot import failed", ex);
                }
                finally
                {
                    lock (_stateGate) _importRunning = false;
                    _importGate.Release();
                }
            });
            return true;
        }
    }

    public Task<object?> HandleAsync(string method, JsonElement? parameters) => method switch
    {
        "status" => Task.FromResult<object?>(Status()),
        "clips.list" => Task.FromResult<object?>(ClipsList(parameters)),
        "clips.calendar" => Task.FromResult<object?>(ClipsCalendar(parameters)),
        "clips.get" => Task.FromResult<object?>(ClipGet(parameters)),
        "clips.copy" => Task.FromResult<object?>(ClipCopy(parameters)),
        "clips.reveal" => Task.FromResult<object?>(ClipReveal(parameters)),
        "clips.pin" => Task.FromResult<object?>(ClipPin(parameters)),
        "clips.delete" => Task.FromResult<object?>(ClipDelete(parameters)),
        "clips.clear" => Task.FromResult<object?>(ClipClear(parameters)),
        "shots.list" => Task.FromResult<object?>(ShotsList(parameters)),
        "shots.get" => Task.FromResult<object?>(ShotGet(parameters)),
        "shots.copy" => Task.FromResult<object?>(ShotCopy(parameters)),
        "shots.delete" => Task.FromResult<object?>(ShotDelete(parameters)),
        "shots.clear" => Task.FromResult<object?>(ShotClear()),
        "shots.reveal" => Task.FromResult<object?>(ShotReveal(parameters)),
        "shots.import" => Task.FromResult<object?>(ShotImport()),
        "storage.reveal" => Task.FromResult<object?>(RevealRoot()),
        "settings.get" => Task.FromResult<object?>(new { settings = _settings, dataDir = _root }),
        "settings.set" => Task.FromResult<object?>(SettingsSet(parameters)),
        "capture.set" => Task.FromResult<object?>(CaptureSet(parameters)),
        "app.info" => Task.FromResult<object?>(new
        {
            id = "thodclip",
            name = "ThodClip",
            version = Version(),
            hubApi = 1,
            dataDir = _root
        }),
        _ => throw new RpcException(-32601, $"Неизвестный метод: {method}")
    };

    private static string Version() =>
        typeof(Engine).Assembly.GetName().Version?.ToString(3) ?? "1.0.0";

    private object Status()
    {
        var stats = _store.GetStats();
        var backgroundRunning = Autostart.IsBackgroundRunning(_root);
        lock (_stateGate)
        {
            return new
            {
                version = Version(),
                hubApi = 1,
                dataDir = _root,
                startedAt = _startedAt,
                captureEnabled = _settings.CaptureEnabled,
                captureActive = _settings.CaptureEnabled &&
                                (_ownsCapture ? _clipboard.IsRunning : backgroundRunning),
                captureOwner = _ownsCapture ? "hub" : backgroundRunning ? "background" : "none",
                runInBackground = _settings.RunInBackground,
                backgroundRunning,
                honorExclusions = _settings.HonorClipboardExclusions,
                dedupeSeconds = _settings.DedupeSeconds,
                folders = ScreenshotScanner.KnownDirectories(),
                clipCount = stats.ClipCount,
                shotCount = stats.ShotCount,
                clipBytes = stats.ClipBytes,
                shotBytes = stats.ShotBytes,
                databaseBytes = stats.DatabaseBytes,
                storageBytes = stats.TotalBytes,
                revision = _store.Revision,
                lastCaptureAt = _lastCaptureMs > 0 ? Iso(_lastCaptureMs) : null,
                lastShotAt = _lastShotMs > 0 ? Iso(_lastShotMs) : null,
                import = new
                {
                    running = _importRunning,
                    scanned = _lastImport?.Scanned ?? 0,
                    imported = _lastImport?.Imported ?? 0,
                    skipped = _lastImport?.Skipped ?? 0,
                    failed = _lastImport?.Failed ?? 0,
                    error = _lastImportError
                },
                limits = new
                {
                    maxClips = _settings.MaxClips,
                    maxTextBytes = _settings.MaxTextBytes,
                    maxScreenshotBytes = _settings.MaxScreenshotBytes
                }
            };
        }
    }

    private ClipPage ClipsList(JsonElement? parameters)
    {
        var query = Str(parameters, "query");
        var kind = Str(parameters, "kind");
        var pinned = Bool(parameters, "pinned");
        var offset = Int(parameters, "offset", 0);
        var limit = Int(parameters, "limit", 40);
        var page = _store.QueryClips(query, kind, pinned, Str(parameters, "date"), offset, limit);
        var fileThumbBudget = 12;
        foreach (var item in page.Items)
        {
            if (item.Kind == "image")
            {
                var thumb = _store.ThumbnailFor(item);
                if (thumb is not null) item.Thumb = DataUrl(thumb, "image/jpeg", 2_000_000);
                continue;
            }
            if (item.Kind != "files" || fileThumbBudget <= 0) continue;
            foreach (var path in item.Files)
            {
                if (!ImageFiles.IsImagePath(path)) continue;
                var thumb = _store.ThumbnailForFile(path);
                if (thumb is null) continue;
                item.Thumb = DataUrl(thumb, "image/jpeg", 1_500_000);
                fileThumbBudget--;
                break;
            }
        }
        return page;
    }

    private ClipCalendar ClipsCalendar(JsonElement? parameters)
    {
        var month = Str(parameters, "month");
        if (string.IsNullOrWhiteSpace(month))
            month = DateTime.Today.ToString("yyyy-MM", System.Globalization.CultureInfo.InvariantCulture);
        try
        {
            return _store.QueryCalendar(month);
        }
        catch (ArgumentException ex)
        {
            throw new RpcException(-32602, ex.Message);
        }
    }

    private object ClipGet(JsonElement? parameters)
    {
        var id = Long(parameters, "id");
        var item = _store.GetClip(id) ?? throw new RpcException(-32004, "Запись не найдена.");
        string? dataUrl = null;
        string? mime = null;
        var preview = false;
        if (item.Kind == "image")
        {
            var path = _store.ResolveBlob(item);
            if (!File.Exists(path)) throw new RpcException(-32004, "Файл изображения не найден.");
            (dataUrl, mime, preview) = ReadImage(path);
        }
        else if (item.Kind == "files")
        {
            var path = item.Files.FirstOrDefault(x => ImageFiles.IsImagePath(x) && File.Exists(x));
            if (path is not null) (dataUrl, mime, preview) = ReadImage(path);
        }
        return new { item, dataUrl, mime, preview };
    }

    private static (string? DataUrl, string? Mime, bool Preview) ReadImage(string path)
    {
        var bytes = File.ReadAllBytes(path);
        if (bytes.Length <= MaxInlineBytes)
        {
            var mime = Path.GetExtension(path).ToLowerInvariant() switch
            {
                ".jpg" or ".jpeg" => "image/jpeg",
                ".gif" => "image/gif",
                ".bmp" => "image/bmp",
                ".webp" => "image/webp",
                ".tif" or ".tiff" => "image/tiff",
                _ => "image/png"
            };
            return ($"data:{mime};base64," + Convert.ToBase64String(bytes), mime, false);
        }
        var jpeg = ImageFiles.Preview(path, 1920, 1440);
        return ("data:image/jpeg;base64," + Convert.ToBase64String(jpeg), "image/jpeg", true);
    }

    private object ClipCopy(JsonElement? parameters)
    {
        var id = Long(parameters, "id");
        var mode = Str(parameters, "mode");
        var item = _store.GetClip(id) ?? throw new RpcException(-32004, "Запись не найдена.");
        switch (item.Kind)
        {
            case "text":
                _clipboard.CopyText(item.Text).GetAwaiter().GetResult();
                break;
            case "files":
                if (mode == "paths")
                {
                    _clipboard.CopyText(item.Text).GetAwaiter().GetResult();
                }
                else
                {
                    var missing = item.Files.Where(x => !File.Exists(x) && !Directory.Exists(x)).ToArray();
                    if (missing.Length > 0)
                        throw new RpcException(-32010, $"Файлы перемещены или удалены ({missing.Length}). Скопируйте пути.");
                    _clipboard.CopyFiles(item.Files).GetAwaiter().GetResult();
                }
                break;
            case "image":
                var path = _store.ResolveBlob(item);
                if (!File.Exists(path)) throw new RpcException(-32004, "Файл изображения не найден.");
                _clipboard.CopyFileImage(path).GetAwaiter().GetResult();
                break;
            default:
                throw new RpcException(-32010, "Этот тип записи нельзя скопировать.");
        }
        return new { ok = true, kind = item.Kind, mode = mode ?? "default" };
    }

    private object ClipReveal(JsonElement? parameters)
    {
        var id = Long(parameters, "id");
        var item = _store.GetClip(id) ?? throw new RpcException(-32004, "Запись не найдена.");
        var path = item.Kind switch
        {
            "image" => _store.ResolveBlob(item),
            "files" => item.Files.FirstOrDefault(File.Exists),
            _ => null
        };
        if (string.IsNullOrWhiteSpace(path) || !File.Exists(path))
            throw new RpcException(-32010, "У этой записи нет доступного файла.");
        Process.Start(new ProcessStartInfo("explorer.exe", $"/select,\"{path}\"") { UseShellExecute = true })?.Dispose();
        return new { ok = true };
    }

    private object ClipPin(JsonElement? parameters)
    {
        var id = Long(parameters, "id");
        var pinned = Bool(parameters, "pinned");
        if (!_store.PinClip(id, pinned)) throw new RpcException(-32004, "Запись не найдена.");
        return new { ok = true, pinned };
    }

    private object ClipDelete(JsonElement? parameters)
    {
        var ids = Ids(parameters);
        if (ids.Count == 0) throw new RpcException(-32602, "Не выбраны записи.");
        var deleted = _store.DeleteClips(ids);
        return new { deleted };
    }

    private object ClipClear(JsonElement? parameters)
    {
        var keepPinned = Bool(parameters, "keepPinned", true);
        var deleted = _store.ClearClips(keepPinned);
        return new { deleted, keepPinned };
    }

    private ShotPage ShotsList(JsonElement? parameters)
    {
        var page = _store.QueryShots(Str(parameters, "query"), Int(parameters, "offset", 0), Int(parameters, "limit", 60));
        foreach (var item in page.Items)
        {
            var thumb = _store.ResolveShotThumb(item);
            if (File.Exists(thumb)) item.Thumb = DataUrl(thumb, "image/jpeg", 1_500_000);
        }
        return page;
    }

    private object ShotGet(JsonElement? parameters)
    {
        var id = Str(parameters, "id") ?? throw new RpcException(-32602, "Не указан снимок.");
        var item = _store.GetShot(id) ?? throw new RpcException(-32004, "Снимок не найден.");
        var path = _store.ResolveShot(item);
        if (!File.Exists(path)) throw new RpcException(-32004, "Файл снимка не найден.");
        var bytes = File.ReadAllBytes(path);
        var extension = Path.GetExtension(path).ToLowerInvariant();
        var mime = extension switch
        {
            ".jpg" or ".jpeg" => "image/jpeg",
            ".gif" => "image/gif",
            ".bmp" => "image/bmp",
            ".webp" => "image/webp",
            ".tif" or ".tiff" => "image/tiff",
            _ => "image/png"
        };
        var preview = false;
        if (bytes.Length <= MaxInlineBytes)
            return new { item, dataUrl = $"data:{mime};base64," + Convert.ToBase64String(bytes), mime, preview, bytes = bytes.Length };
        var jpeg = ImageFiles.Preview(path, 1920, 1440);
        var dataUrl = "data:image/jpeg;base64," + Convert.ToBase64String(jpeg);
        return new { item, dataUrl, mime = "image/jpeg", preview = true, bytes = bytes.Length };
    }

    private object ShotCopy(JsonElement? parameters)
    {
        var id = Str(parameters, "id") ?? throw new RpcException(-32602, "Не указан снимок.");
        var item = _store.GetShot(id) ?? throw new RpcException(-32004, "Снимок не найден.");
        var path = _store.ResolveShot(item);
        if (!File.Exists(path)) throw new RpcException(-32004, "Файл снимка не найден.");
        _clipboard.CopyFileImage(path).GetAwaiter().GetResult();
        return new { ok = true };
    }

    private object ShotDelete(JsonElement? parameters)
    {
        var ids = StrList(parameters, "ids");
        if (ids.Count == 0) throw new RpcException(-32602, "Не выбраны снимки.");
        return new { deleted = _store.DeleteShots(ids) };
    }

    private object ShotClear()
    {
        var deleted = _store.ClearShots();
        _lastShotMs = 0;
        return new { deleted };
    }

    private object ShotReveal(JsonElement? parameters)
    {
        var id = Str(parameters, "id");
        if (string.IsNullOrWhiteSpace(id)) return RevealRoot();
        var item = _store.GetShot(id) ?? throw new RpcException(-32004, "Снимок не найден.");
        var path = _store.ResolveShot(item);
        if (!File.Exists(path)) throw new RpcException(-32004, "Файл снимка не найден.");
        Process.Start(new ProcessStartInfo("explorer.exe", $"/select,\"{path}\"") { UseShellExecute = true })?.Dispose();
        return new { ok = true };
    }

    private object RevealRoot()
    {
        Process.Start(new ProcessStartInfo(_root) { UseShellExecute = true })?.Dispose();
        return new { ok = true, path = _root };
    }

    private object ShotImport()
    {
        var started = StartImport(force: true);
        return new { started, message = started ? "Импорт запущен." : "Импорт уже выполняется." };
    }

    private object SettingsSet(JsonElement? parameters)
    {
        var source = parameters ?? throw new RpcException(-32602, "Нет настроек.");
        var next = _settings.Clone();
        if (Try(source, "captureEnabled", out var capture)) next.CaptureEnabled = capture.ValueKind == JsonValueKind.True;
        if (Try(source, "honorClipboardExclusions", out var honor)) next.HonorClipboardExclusions = honor.ValueKind == JsonValueKind.True;
        if (Try(source, "autoImportScreenshots", out var auto)) next.AutoImportScreenshots = auto.ValueKind == JsonValueKind.True;
        if (Try(source, "runInBackground", out var run)) next.RunInBackground = run.ValueKind == JsonValueKind.True;
        if (Try(source, "dedupeSeconds", out var dedupe) && dedupe.TryGetInt32(out var dedupeValue)) next.DedupeSeconds = dedupeValue;
        if (Try(source, "maxClips", out var maxClips) && maxClips.TryGetInt32(out var maxClipsValue)) next.MaxClips = maxClipsValue;
        if (Try(source, "maxTextBytes", out var maxText) && maxText.TryGetInt32(out var maxTextValue)) next.MaxTextBytes = maxTextValue;
        if (Try(source, "maxScreenshotBytes", out var maxShots) && maxShots.TryGetInt64(out var maxShotsValue)) next.MaxScreenshotBytes = maxShotsValue;
        next.Sanitize();
        var backgroundChanged = next.RunInBackground != _settings.RunInBackground;
        SettingsStore.Save(_root, next);
        ApplySettings(next);
        _store.PruneNow();
        if (backgroundChanged)
        {
            try
            {
                Autostart.Apply(next.RunInBackground, _root);
            }
            catch (Exception ex)
            {
                if (next.RunInBackground)
                {
                    next.RunInBackground = false;
                    SettingsStore.Save(_root, next);
                    ApplySettings(next);
                    try { Autostart.Apply(false, _root); } catch { }
                }
                throw new RpcException(-32000, "Не удалось настроить фоновый режим: " + ex.Message);
            }
        }
        return new { settings = _settings, dataDir = _root };
    }

    private object CaptureSet(JsonElement? parameters)
    {
        var enabled = Bool(parameters, "enabled", _settings.CaptureEnabled);
        _settings.CaptureEnabled = enabled;
        SettingsStore.Save(_root, _settings);
        ApplyCaptureGating();
        return new { captureEnabled = enabled };
    }

    /// <summary>Applies a settings file changed by another process while the background worker runs.</summary>
    public void ReloadSettings(ClipSettings next)
    {
        var autoImport = !_settings.AutoImportScreenshots && next.AutoImportScreenshots;
        ApplySettings(next);
        if (autoImport && _ownsCapture) StartImport();
        _store.PruneNow();
    }

    private void ApplySettings(ClipSettings next)
    {
        _settings.CaptureEnabled = next.CaptureEnabled;
        _settings.HonorClipboardExclusions = next.HonorClipboardExclusions;
        _settings.AutoImportScreenshots = next.AutoImportScreenshots;
        _settings.RunInBackground = next.RunInBackground;
        _settings.DedupeSeconds = next.DedupeSeconds;
        _settings.MaxClips = next.MaxClips;
        _settings.MaxTextBytes = next.MaxTextBytes;
        _settings.MaxScreenshotBytes = next.MaxScreenshotBytes;
        ApplyCaptureGating();
    }

    private static string? DataUrl(string path, string mime, long limit)
    {
        try
        {
            var info = new FileInfo(path);
            if (!info.Exists || info.Length > limit) return null;
            return $"data:{mime};base64," + Convert.ToBase64String(File.ReadAllBytes(path));
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { return null; }
    }

    private static bool Try(JsonElement source, string name, out JsonElement value)
    {
        value = default;
        return source.ValueKind == JsonValueKind.Object && source.TryGetProperty(name, out value);
    }

    private static string? Str(JsonElement? parameters, string name) =>
        parameters is { ValueKind: JsonValueKind.Object } p && p.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String
            ? value.GetString()
            : null;

    private static int Int(JsonElement? parameters, string name, int fallback) =>
        parameters is { ValueKind: JsonValueKind.Object } p && p.TryGetProperty(name, out var value) && value.TryGetInt32(out var number)
            ? number
            : fallback;

    private static long Long(JsonElement? parameters, string name) =>
        parameters is { ValueKind: JsonValueKind.Object } p && p.TryGetProperty(name, out var value) && value.TryGetInt64(out var number)
            ? number
            : throw new RpcException(-32602, $"Не указан параметр {name}.");

    private static bool Bool(JsonElement? parameters, string name, bool fallback = false) =>
        parameters is { ValueKind: JsonValueKind.Object } p && p.TryGetProperty(name, out var value) &&
        value.ValueKind is JsonValueKind.True or JsonValueKind.False
            ? value.GetBoolean()
            : fallback;

    private static List<long> Ids(JsonElement? parameters)
    {
        var result = new List<long>();
        if (parameters is not { ValueKind: JsonValueKind.Object } p) return result;
        if (!p.TryGetProperty("ids", out var ids) || ids.ValueKind != JsonValueKind.Array) return result;
        foreach (var item in ids.EnumerateArray())
            if (item.TryGetInt64(out var id)) result.Add(id);
        return result;
    }

    private static List<string> StrList(JsonElement? parameters, string name)
    {
        var result = new List<string>();
        if (parameters is not { ValueKind: JsonValueKind.Object } p) return result;
        if (!p.TryGetProperty(name, out var values) || values.ValueKind != JsonValueKind.Array) return result;
        foreach (var item in values.EnumerateArray())
            if (item.ValueKind == JsonValueKind.String && item.GetString() is { Length: > 0 } value) result.Add(value);
        return result;
    }

    public async ValueTask DisposeAsync()
    {
        await _lifetime.CancelAsync();
        _captureLease.Dispose();
        _ownsCapture = false;
        _clipboard.Dispose();
        _screenshots.Dispose();
        try
        {
            if (_importTask is not null) await _importTask.WaitAsync(TimeSpan.FromSeconds(2));
        }
        catch (Exception ex) when (ex is TimeoutException or OperationCanceledException) { }
        _store.Dispose();
        _importGate.Dispose();
        _lifetime.Dispose();
    }
}

internal sealed class RpcException(int code, string message) : Exception(message)
{
    public int Code { get; } = code;
}
