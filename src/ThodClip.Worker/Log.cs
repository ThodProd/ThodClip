using System.Text;

namespace ThodClip.Worker;

/// <summary>
/// Diagnostics go to stderr for the hub and to a small local file for the background worker,
/// which has no console of its own.
/// </summary>
internal static class Log
{
    private const long MaxFileBytes = 2 * 1024 * 1024;
    private static readonly object Gate = new();
    private static string? _filePath;

    public static void ConfigureFile(string? path)
    {
        lock (Gate)
        {
            _filePath = path;
            if (string.IsNullOrWhiteSpace(path)) return;
            try
            {
                var directory = Path.GetDirectoryName(path);
                if (!string.IsNullOrWhiteSpace(directory)) Directory.CreateDirectory(directory);
                var info = new FileInfo(path);
                if (info.Exists && info.Length > MaxFileBytes) info.Delete();
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                _filePath = null;
            }
        }
    }

    public static void Info(string message) => Write("info", message);

    public static void Warn(string message) => Write("warn", message);

    public static void Error(string message, Exception? ex = null) =>
        Write("error", ex is null ? message : message + ": " + ex.Message);

    private static void Write(string level, string message)
    {
        var line = $"[{DateTimeOffset.Now:yyyy-MM-dd HH:mm:ss}] [{level}] {message}";
        lock (Gate)
        {
            try { Console.Error.WriteLine(line); }
            catch (Exception ex) when (ex is IOException or ObjectDisposedException) { }

            if (string.IsNullOrWhiteSpace(_filePath)) return;
            try
            {
                using var stream = new FileStream(_filePath, FileMode.Append, FileAccess.Write,
                    FileShare.ReadWrite | FileShare.Delete);
                using var writer = new StreamWriter(stream, new UTF8Encoding(false));
                writer.WriteLine(line);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ObjectDisposedException) { }
        }
    }
}
