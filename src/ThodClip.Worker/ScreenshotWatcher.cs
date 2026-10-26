using System.Collections.Concurrent;
using System.Threading.Channels;
using ThodClip.Core;

namespace ThodClip.Worker;

/// <summary>Watches the usual screenshot folders and copies new images into the archive.</summary>
internal sealed class ScreenshotWatcher : IDisposable
{
    private readonly ClipStore _store;
    private readonly List<FileSystemWatcher> _watchers = new();
    private readonly Channel<string> _queue = Channel.CreateUnbounded<string>(new UnboundedChannelOptions { SingleReader = true });
    private readonly ConcurrentDictionary<string, long> _seen = new(StringComparer.OrdinalIgnoreCase);
    private readonly CancellationTokenSource _lifetime = new();
    private Task? _consumer;

    public event Action<string>? Imported;

    public ScreenshotWatcher(ClipStore store) => _store = store;

    public IReadOnlyList<string> Folders => ScreenshotScanner.KnownDirectories();

    public void Start()
    {
        foreach (var folder in Folders)
        {
            try
            {
                var watcher = new FileSystemWatcher(folder, "*")
                {
                    IncludeSubdirectories = false,
                    NotifyFilter = NotifyFilters.FileName | NotifyFilters.Size | NotifyFilters.LastWrite,
                    EnableRaisingEvents = true
                };
                watcher.Created += (_, e) => Enqueue(e.FullPath);
                watcher.Renamed += (_, e) => Enqueue(e.FullPath);
                watcher.Changed += (_, e) => Enqueue(e.FullPath);
                watcher.Error += (_, e) => Log.Warn("screenshot watcher: " + e.GetException().Message);
                _watchers.Add(watcher);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException)
            {
                Log.Warn($"cannot watch {folder}: {ex.Message}");
            }
        }
        _consumer = Task.Run(ConsumeAsync);
        Log.Info($"watching {_watchers.Count} screenshot folders");
    }

    private void Enqueue(string path)
    {
        if (!ImageFiles.IsImagePath(path) || !ScreenshotScanner.IsScreenshot(path)) return;
        var now = Environment.TickCount64;
        if (_seen.TryGetValue(path, out var last) && now - last < 4000) return;
        _seen[path] = now;
        _queue.Writer.TryWrite(path);
    }

    private async Task ConsumeAsync()
    {
        try
        {
            await foreach (var path in _queue.Reader.ReadAllAsync(_lifetime.Token))
            {
                try
                {
                    if (!ScreenshotScanner.IsScreenshot(path)) continue;
                    if (!ScreenshotScanner.IsStable(path, attempts: 8)) continue;
                    var folder = Path.GetFileName(Path.GetDirectoryName(path) ?? "") ?? "Скриншоты";
                    var id = _store.AddScreenshotFile(path, folder);
                    if (!string.IsNullOrEmpty(id))
                    {
                        Log.Info($"archived screenshot {Path.GetFileName(path)}");
                        Imported?.Invoke(id);
                    }
                }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidDataException)
                {
                    Log.Warn("screenshot archive failed: " + ex.Message);
                }
            }
        }
        catch (OperationCanceledException) { }
        catch (Exception ex)
        {
            Log.Error("screenshot watcher stopped", ex);
        }
    }

    public void Stop()
    {
        _lifetime.Cancel();
        _queue.Writer.TryComplete();
        foreach (var watcher in _watchers)
        {
            try { watcher.EnableRaisingEvents = false; watcher.Dispose(); } catch (Exception ex) when (ex is IOException or ObjectDisposedException) { }
        }
        _watchers.Clear();
        try { _consumer?.Wait(2500); } catch (AggregateException) { }
    }

    public void Dispose() => Stop();
}
