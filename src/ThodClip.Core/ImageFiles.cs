using System.Drawing;
using System.Drawing.Drawing2D;
using System.Drawing.Imaging;
using System.Security.Cryptography;
using System.Text;

namespace ThodClip.Core;

public static class ImageFiles
{
    private const long MaxPixels = 120_000_000;
    public static readonly string[] Extensions = [".png", ".jpg", ".jpeg", ".bmp", ".gif", ".webp", ".tif", ".tiff"];

    public static string HashBytes(ReadOnlySpan<byte> data) => Convert.ToHexString(SHA256.HashData(data)).ToLowerInvariant();

    public static string HashString(string value) => HashBytes(Encoding.UTF8.GetBytes(value));

    public static bool IsImagePath(string path) =>
        Extensions.Contains(Path.GetExtension(path), StringComparer.OrdinalIgnoreCase);

    public static bool TryReadInfo(string path, out int width, out int height)
    {
        width = height = 0;
        try
        {
            using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
            using var image = Image.FromStream(stream, useEmbeddedColorManagement: false, validateImageData: false);
            if ((long)image.Width * image.Height > MaxPixels) return false;
            width = image.Width;
            height = image.Height;
            return width > 0 && height > 0;
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or OutOfMemoryException)
        {
            return false;
        }
    }

    public static byte[] ToPng(Image image)
    {
        using var stream = new MemoryStream();
        image.Save(stream, ImageFormat.Png);
        return stream.ToArray();
    }

    public static byte[] Thumbnail(string path, int maxWidth = 560, int maxHeight = 420)
    {
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        using var image = Image.FromStream(stream, useEmbeddedColorManagement: false, validateImageData: false);
        return Thumbnail(image, maxWidth, maxHeight);
    }

    public static byte[] Thumbnail(Image image, int maxWidth = 560, int maxHeight = 420) =>
        ResizeJpeg(image, maxWidth, maxHeight, 82);

    public static byte[] Preview(string path, int maxWidth = 1920, int maxHeight = 1440) =>
        ResizeJpegFromPath(path, maxWidth, maxHeight, 88);

    public static byte[] ResizeJpegFromPath(string path, int maxWidth, int maxHeight, long quality)
    {
        using var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete);
        using var image = Image.FromStream(stream, useEmbeddedColorManagement: false, validateImageData: false);
        return ResizeJpeg(image, maxWidth, maxHeight, quality);
    }

    public static byte[] ResizeJpeg(Image image, int maxWidth, int maxHeight, long quality)
    {
        var scale = Math.Min(1d, Math.Min((double)maxWidth / image.Width, (double)maxHeight / image.Height));
        var width = Math.Max(1, (int)Math.Round(image.Width * scale));
        var height = Math.Max(1, (int)Math.Round(image.Height * scale));
        using var canvas = new Bitmap(width, height, PixelFormat.Format24bppRgb);
        using (var graphics = Graphics.FromImage(canvas))
        {
            graphics.Clear(Color.FromArgb(15, 18, 25));
            graphics.InterpolationMode = InterpolationMode.HighQualityBicubic;
            graphics.PixelOffsetMode = PixelOffsetMode.HighQuality;
            graphics.SmoothingMode = SmoothingMode.HighQuality;
            graphics.DrawImage(image, 0, 0, width, height);
        }
        using var output = new MemoryStream();
        var encoder = ImageCodecInfo.GetImageEncoders().FirstOrDefault(x => x.FormatID == ImageFormat.Jpeg.Guid);
        if (encoder is null)
        {
            canvas.Save(output, ImageFormat.Jpeg);
        }
        else
        {
            using var parameters = new EncoderParameters(1);
            parameters.Param[0] = new EncoderParameter(System.Drawing.Imaging.Encoder.Quality, Math.Clamp(quality, 40, 100));
            canvas.Save(output, encoder, parameters);
        }
        return output.ToArray();
    }
}
