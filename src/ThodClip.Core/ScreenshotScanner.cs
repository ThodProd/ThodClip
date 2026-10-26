using System.Text.RegularExpressions;

namespace ThodClip.Core;

/// <summary>Finds screenshots in the folders Windows and Snipping Tool normally use.</summary>
public static partial class ScreenshotScanner
{
    [GeneratedRegex(@"(?ix)(screenshot|screen[\s_-]?shot|snip|снимок(\s+экрана)?|скрин(шот)?|print[\s_-]?screen|screencap|capture)")]
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
        foreach (var extra in ExtraDirectories())
        {
            if (Directory.Exists(extra)) Add(extra);
        }
        return candidates;
    }

    public static IReadOnlyList<string> ExtraDirectories() =>
        (Environment.GetEnvironmentVariable("THODCLIP_SCREENSHOT_DIRS") ?? "")
        .Split(';', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
        .Where(Directory.Exists)
        .ToArray();

    public static bool IsScreenshot(string path)
    {
        if (!File.Exists(path) || !ImageFiles.IsImagePath(path)) return false;
        var full = Path.GetFullPath(path);
        if (ExtraDirectories().Any(d => IsInside(d, full))) return true;
        var parent = Path.GetFileName(Path.GetDirectoryName(full) ?? "");
        if (parent.Equals("Screenshots", StringComparison.OrdinalIgnoreCase)) return true;
        return ScreenshotName().IsMatch(Path.GetFileNameWithoutExtension(path));
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
