using System.Text.RegularExpressions;

namespace ThodClip.Core;

/// <summary>Finds screenshots in the folders Windows and Snipping Tool normally use.</summary>
public static partial class ScreenshotScanner
{
    [GeneratedRegex(@"(?ix)(screenshot|screen[\s_-]?shot|screen[\s_-]?clip|screen[\s_-]?snip|snip|sketch|снимок(\s+экрана)?|скрин(шот)?|print[\s_-]?screen|screencap|capture)")]
    private static partial Regex ScreenshotName();

    public static IReadOnlyList<string> KnownDirectories()
    {
        if (Environment.GetEnvironmentVariable("THODCLIP_SCREENSHOT_ONLY") == "1")
            return ExtraDirectories();

        var profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        var candidates = new List<string>();
        void Add(string? path)
        {
            if (string.IsNullOrWhiteSpace(path)) return;
            try
            {
                var full = Path.GetFullPath(path);
                if (Directory.Exists(full) && !candidates.Contains(full, StringComparer.OrdinalIgnoreCase))
                    candidates.Add(full);
            }
            catch (Exception ex) when (ex is ArgumentException or IOException or UnauthorizedAccessException) { }
        }
        Add(Path.Combine(profile, "Pictures", "Screenshots"));
        Add(Path.Combine(profile, "OneDrive", "Pictures", "Screenshots"));
        Add(Path.Combine(profile, "Desktop"));
        Add(Path.Combine(profile, "Pictures"));
        Add(Path.Combine(profile, "OneDrive", "Pictures"));
        Add(Path.Combine(profile, "Downloads"));
        foreach (var screenClip in ScreenClipDirectories())
        {
            Add(screenClip);
        }
        foreach (var extra in ExtraDirectories())
        {
            if (Directory.Exists(extra)) Add(extra);
        }
        return candidates;
    }

    /// <summary>
    /// Snipping Tool keeps its clippings in a temporary package folder. Watching it as well means
    /// a screenshot is archived even when the clipboard delivers only a temporary file path.
    /// </summary>
    private static IEnumerable<string> ScreenClipDirectories()
    {
        var packages = Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "Packages");
        if (!Directory.Exists(packages)) yield break;

        string[] entries;
        try
        {
            entries = Directory.GetDirectories(packages);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            yield break;
        }

        foreach (var package in entries)
        {
            var name = Path.GetFileName(package);
            var isScreenSketch = name.StartsWith("Microsoft.ScreenSketch_", StringComparison.OrdinalIgnoreCase);
            var isSnippingTool = name.StartsWith("Microsoft.SnippingTool_", StringComparison.OrdinalIgnoreCase);
            var isClientCore = name.StartsWith("MicrosoftWindows.Client.Core_", StringComparison.OrdinalIgnoreCase);
            if (!isScreenSketch && !isSnippingTool && !isClientCore) continue;

            var temp = Path.Combine(package, "TempState");
            if (isScreenSketch || isSnippingTool) yield return temp;
            var screenClip = Path.Combine(temp, "ScreenClip");
            if (Directory.Exists(screenClip)) yield return screenClip;
        }
    }

    public static IReadOnlyList<string> ExtraDirectories() =>
        (Environment.GetEnvironmentVariable("THODCLIP_SCREENSHOT_DIRS") ?? "")
        .Split(';', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
        .Where(Directory.Exists)
        .ToArray();

    public static bool IsScreenshot(string path)
    {
        if (!File.Exists(path) || !ImageFiles.IsImagePath(path)) return false;
        return IsScreenshotPath(path);
    }

    /// <summary>Classifies a path without opening the file, so a temporary clipboard path still works.</summary>
    public static bool IsScreenshotPath(string path)
    {
        if (string.IsNullOrWhiteSpace(path) || !ImageFiles.IsImagePath(path)) return false;
        string full;
        try { full = Path.GetFullPath(path); }
        catch (Exception ex) when (ex is ArgumentException or NotSupportedException or PathTooLongException)
        {
            return false;
        }
        if (ExtraDirectories().Any(d => IsInside(d, full))) return true;
        if (IsScreenClipPath(full)) return true;

        var parent = Path.GetFileName(Path.GetDirectoryName(full) ?? "");
        if (parent.Equals("Screenshots", StringComparison.OrdinalIgnoreCase) ||
            parent.Equals("ScreenClip", StringComparison.OrdinalIgnoreCase) ||
            parent.Equals("ScreenClips", StringComparison.OrdinalIgnoreCase) ||
            parent.Equals("ScreenSketch", StringComparison.OrdinalIgnoreCase))
            return true;
        return ScreenshotName().IsMatch(Path.GetFileNameWithoutExtension(path));
    }

    private static bool IsScreenClipPath(string full)
    {
        var directory = Path.GetDirectoryName(full) ?? "";
        if (directory.Contains(@"\TempState\ScreenClip", StringComparison.OrdinalIgnoreCase)) return true;
        if (directory.Contains(@"\TempState\ScreenSketch", StringComparison.OrdinalIgnoreCase)) return true;
        if (directory.Contains(@"\Microsoft.ScreenSketch_", StringComparison.OrdinalIgnoreCase) &&
            directory.Contains(@"\TempState", StringComparison.OrdinalIgnoreCase)) return true;
        if (directory.Contains(@"\Microsoft.SnippingTool_", StringComparison.OrdinalIgnoreCase) &&
            directory.Contains(@"\TempState", StringComparison.OrdinalIgnoreCase)) return true;
        return false;
    }

    public static ImportSummary Import(ClipStore store, CancellationToken cancellationToken)
    {
        var folders = KnownDirectories();
        var scanned = 0;
        var failed = 0;
        var before = store.GetStats().ShotCount;
        foreach (var folder in folders)
        {
            cancellationToken.ThrowIfCancellationRequested();
            List<string> files;
            try
            {
                files = Directory.EnumerateFiles(folder, "*", SearchOption.TopDirectoryOnly)
                    .Where(IsScreenshot)
                    .OrderByDescending(File.GetLastWriteTimeUtc)
                    .Take(5000)
                    .ToList();
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { continue; }
            foreach (var file in files)
            {
                cancellationToken.ThrowIfCancellationRequested();
                if (IsInside(store.Root, file)) continue;
                scanned++;
                try
                {
                    if (!IsStable(file, attempts: 2)) { failed++; continue; }
                    store.AddScreenshotFile(file, FolderLabel(folder));
                }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidDataException) { failed++; }
            }
        }
        var after = store.GetStats().ShotCount;
        return new ImportSummary
        {
            Scanned = scanned,
            Imported = Math.Max(0, after - before),
            Failed = failed,
            Skipped = Math.Max(0, scanned - (after - before) - failed),
            Folders = folders
        };
    }

    private static string FolderLabel(string folder)
    {
        var name = Path.GetFileName(folder.TrimEnd(Path.DirectorySeparatorChar));
        return string.IsNullOrWhiteSpace(name) ? folder : name;
    }

    private static bool IsInside(string directory, string file)
    {
        try
        {
            var root = Path.GetFullPath(directory).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
            return Path.GetFullPath(file).StartsWith(root, StringComparison.OrdinalIgnoreCase);
        }
        catch (Exception ex) when (ex is ArgumentException or IOException or UnauthorizedAccessException)
        {
            return false;
        }
    }

    /// <summary>Waits briefly until a freshly created file stops growing and can be opened.</summary>
    public static bool IsStable(string path, int attempts = 6)
    {
        long previous = -1;
        for (var i = 0; i < attempts; i++)
        {
            try
            {
                var info = new FileInfo(path);
                if (!info.Exists) return false;
                if (info.Length == previous && info.Length > 0)
                {
                    using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
                    return stream.Length > 0;
                }
                previous = info.Length;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { }
            Thread.Sleep(180);
        }
        return false;
    }
}
