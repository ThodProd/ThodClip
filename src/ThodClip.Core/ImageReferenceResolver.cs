using System.Drawing;
using System.Net;
using System.Text.RegularExpressions;

namespace ThodClip.Core;

/// <summary>
/// Resolves clipboard text such as "{GUID}.png" or an HTML &lt;img&gt; tag to a real local
/// picture. Windows and some WebView2 apps copy a temporary picture name instead of bitmap data.
/// </summary>
public static partial class ImageReferenceResolver
{
    private const int MaxReferenceLength = 1024;
    private const int MaxHtmlLength = 4_000_000;

    [GeneratedRegex(@"(?i)\.(png|jpe?g|bmp|gif|webp|tif|tiff)$", RegexOptions.CultureInvariant)]
    private static partial Regex ImageName();

    [GeneratedRegex(@"(?is)<img\b[^>]*\bsrc\s*=\s*(?<q>[""'])(?<src>.+?)\k<q>")]
    private static partial Regex HtmlImage();

    [GeneratedRegex(@"(?is)(?:src|href)\s*=\s*(?<q>[""'])(?<src>file:///[^""']+)\k<q>")]
    private static partial Regex HtmlFileUrl();

    public static bool LooksLikeImageReference(string? value)
    {
        if (string.IsNullOrWhiteSpace(value)) return false;
        var text = value.Trim().Trim('"', '\'', '<', '>').Trim();
        if (text.Length is 0 or > MaxReferenceLength) return false;
        if (text.Contains('\n') || text.Contains('\r') || text.Contains('\t')) return false;
        if (text.StartsWith("data:image/", StringComparison.OrdinalIgnoreCase)) return false;
        return ImageName().IsMatch(text);
    }

    /// <summary>Finds an existing image file for a path, file URI or a bare file name.</summary>
    public static bool TryResolve(string? reference, out string path, IReadOnlyList<string>? extraDirectories = null)
    {
        path = "";
        if (string.IsNullOrWhiteSpace(reference)) return false;
        var text = Clean(reference);
        if (text.Length is 0 or > MaxReferenceLength) return false;
        if (text.Contains('\n') || text.Contains('\r') || text.Contains('\t')) return false;
        if (text.Contains("://", StringComparison.Ordinal) &&
            !text.StartsWith("file:", StringComparison.OrdinalIgnoreCase)) return false;

        // A full path can be used directly. A bare file name is searched in known folders only,
        // so a random "{GUID}.png" does not accidentally resolve against the process directory.
        var hasDirectory = text.IndexOfAny(['\\', '/']) >= 0;
        if (hasDirectory && TryExisting(text, out path)) return true;

        var name = Path.GetFileName(text.Replace('/', '\\'));
        if (string.IsNullOrWhiteSpace(name) || !ImageName().IsMatch(name)) return false;

        foreach (var directory in Directories(extraDirectories))
        {
            try
            {
                var candidate = Path.Combine(directory, name);
                if (TryExisting(candidate, out path)) return true;
            }
            catch (Exception ex) when (ex is ArgumentException or IOException or UnauthorizedAccessException) { }
        }
        return false;
    }

    /// <summary>Extracts a local file or an embedded data URL from the HTML clipboard format.</summary>
    public static bool TryResolveHtmlImage(string? html, out string? path, out byte[]? png,
        out int width, out int height)
    {
        path = null;
        png = null;
        width = height = 0;
        if (string.IsNullOrWhiteSpace(html) || html.Length > MaxHtmlLength) return false;

        var fragment = ExtractFragment(html);
        // If the selection contains real text, it is a text copy that happens to include an
        // image (page icons, emoji, previews). Do not replace the text with a random picture.
        if (HasMeaningfulText(fragment)) return false;

        foreach (Match match in HtmlImage().Matches(fragment))
        {
            var source = WebUtility.HtmlDecode(match.Groups["src"].Value).Trim();
            if (source.StartsWith("data:image/", StringComparison.OrdinalIgnoreCase))
            {
                if (TryDecodeDataUrl(source, out png, out width, out height)) return true;
                continue;
            }
            if (TryResolve(source, out var resolved) &&
                TryEncode(resolved, out png, out width, out height))
            {
                path = resolved;
                return true;
            }
        }

        foreach (Match match in HtmlFileUrl().Matches(fragment))
        {
            var source = WebUtility.HtmlDecode(match.Groups["src"].Value).Trim();
            if (TryResolve(source, out var resolved) &&
                TryEncode(resolved, out png, out width, out height))
            {
                path = resolved;
                return true;
            }
        }
        return false;
    }

    private static string ExtractFragment(string html)
    {
        var start = html.IndexOf("<!--StartFragment-->", StringComparison.OrdinalIgnoreCase);
        var end = html.IndexOf("<!--EndFragment-->", StringComparison.OrdinalIgnoreCase);
        if (start >= 0 && end > start) return html[(start + 18)..end];
        return html;
    }

    private static bool HasMeaningfulText(string fragment)
    {
        var text = Regex.Replace(fragment, @"(?is)<(script|style)\b.*?</\1>", " ");
        text = Regex.Replace(text, @"<[^>]+>", " ");
        text = WebUtility.HtmlDecode(text);
        return text.Any(char.IsLetterOrDigit);
    }

    private static string Clean(string value)
    {
        var text = value.Trim().Trim('"', '\'', '<', '>').Trim();
        if (!text.StartsWith("file:", StringComparison.OrdinalIgnoreCase)) return text;
        try
        {
            return new Uri(text).LocalPath;
        }
        catch (Exception ex) when (ex is UriFormatException or ArgumentException)
        {
            return text;
        }
    }

    private static bool TryExisting(string value, out string path)
    {
        path = "";
        try
        {
            var full = Path.GetFullPath(value);
            if (!File.Exists(full) || !ImageFiles.IsImagePath(full)) return false;
            path = full;
            return true;
        }
        catch (Exception ex) when (ex is ArgumentException or IOException or UnauthorizedAccessException
            or NotSupportedException or PathTooLongException)
        {
            return false;
        }
    }

    private static bool TryEncode(string path, out byte[]? png, out int width, out int height)
    {
        png = ImageFiles.TryEncodeFileAsPng(path, out width, out height);
        return png is not null;
    }

    private static bool TryDecodeDataUrl(string source, out byte[]? png, out int width, out int height)
    {
        png = null;
        width = height = 0;
        try
        {
            var comma = source.IndexOf(',');
            if (comma < 0) return false;
            var header = source[..comma];
            if (!header.Contains(";base64", StringComparison.OrdinalIgnoreCase)) return false;
            var bytes = Convert.FromBase64String(source[(comma + 1)..]);
            using var stream = new MemoryStream(bytes);
            using var image = Image.FromStream(stream, useEmbeddedColorManagement: false, validateImageData: false);
            if (image.Width <= 0 || image.Height <= 0) return false;
            if ((long)image.Width * image.Height > 120_000_000) return false;
            width = image.Width;
            height = image.Height;
            png = ImageFiles.ToPng(image);
            return true;
        }
        catch (Exception ex) when (ex is FormatException or ArgumentException or IOException
            or OutOfMemoryException or NotSupportedException)
        {
            return false;
        }
    }

    private static IEnumerable<string> Directories(IReadOnlyList<string>? extraDirectories)
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var result = new List<string>();

        void Add(string? value)
        {
            if (string.IsNullOrWhiteSpace(value)) return;
            try
            {
                var full = Path.GetFullPath(value);
                if (Directory.Exists(full) && seen.Add(full)) result.Add(full);
            }
            catch (Exception ex) when (ex is ArgumentException or IOException or UnauthorizedAccessException
                or NotSupportedException or PathTooLongException) { }
        }

        var local = Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData);
        var profile = Environment.GetFolderPath(Environment.SpecialFolder.UserProfile);
        Add(Path.GetTempPath());
        Add(Path.Combine(local, "Temp"));
        Add(Path.Combine(profile, "Pictures"));
        Add(Path.Combine(profile, "Desktop"));
        Add(Path.Combine(profile, "Downloads"));
        Add(Path.Combine(profile, "OneDrive", "Pictures"));
        Add(Path.Combine(profile, "OneDrive", "Рабочий стол"));

        var packages = Path.Combine(local, "Packages");
        foreach (var pattern in new[]
                 {
                     "MicrosoftWindows.Client.Core_*",
                     "Microsoft.ScreenSketch_*",
                     "MicrosoftWindows.Client.CBS_*"
                 })
        {
            try
            {
                foreach (var package in Directory.EnumerateDirectories(packages, pattern))
                {
                    Add(Path.Combine(package, "TempState", "ScreenClip"));
                    Add(Path.Combine(package, "TempState"));
                    Add(Path.Combine(package, "LocalState"));
                }
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException
                or DirectoryNotFoundException) { }
        }

        foreach (var directory in ScreenshotScanner.KnownDirectories()) Add(directory);
        if (extraDirectories is not null)
        {
            foreach (var directory in extraDirectories) Add(directory);
        }
        return result;
    }
}
