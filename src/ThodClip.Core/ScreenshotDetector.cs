namespace ThodClip.Core;

/// <summary>
/// Decides whether an image taken from the clipboard is a screenshot that belongs in the archive.
/// Windows often reports <c>svchost</c> as the clipboard owner and supplies a temporary
/// <c>ScreenClip</c> file, so size alone is not enough for partial Snipping Tool captures.
/// </summary>
public static class ScreenshotDetector
{
    private static readonly string[] SourceHints =
    [
        "snip", "screensketch", "screenclip", "screenshot", "screenpresso",
        "sharex", "lightshot", "greenshot", "flameshot", "picpick", "gyazo"
    ];

    public static bool IsLikelyScreenshot(
        IEnumerable<string>? files,
        string? source,
        int width,
        int height,
        int screenWidth,
        int screenHeight,
        bool archiveAllImages = false)
    {
        if (archiveAllImages) return true;
        if (width <= 0 || height <= 0) return false;

        if (files is not null)
        {
            foreach (var file in files)
            {
                if (string.IsNullOrWhiteSpace(file)) continue;
                if (ScreenshotScanner.IsScreenshotPath(file)) return true;
            }
        }

        var owner = (source ?? "").ToLowerInvariant();
        if (SourceHints.Any(hint => owner.Contains(hint, StringComparison.Ordinal))) return true;

        if (screenWidth <= 0 || screenHeight <= 0) return false;
        var sameSize = Math.Abs(width - screenWidth) <= 2 && Math.Abs(height - screenHeight) <= 2;
        var sameArea = (long)width * height >= (long)screenWidth * screenHeight * 85 / 100;
        return sameSize || sameArea;
    }
}
