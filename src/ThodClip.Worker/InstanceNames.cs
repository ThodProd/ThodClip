using System.Security.Cryptography;
using System.Text;

namespace ThodClip.Worker;

internal static class InstanceNames
{
    private static string Key(string dataDir)
    {
        var normalized = Path.GetFullPath(dataDir).TrimEnd(Path.DirectorySeparatorChar).ToUpperInvariant();
        return Convert.ToHexString(SHA256.HashData(Encoding.UTF8.GetBytes(normalized)))[..16];
    }

    public static string CaptureMutex(string dataDir) => $@"Local\ThodClip.Capture.{Key(dataDir)}";
    public static string BackgroundMutex(string dataDir) => $@"Local\ThodClip.Background.{Key(dataDir)}";
    public static string StopEvent(string dataDir) => $@"Local\ThodClip.Background.Stop.{Key(dataDir)}";
}
