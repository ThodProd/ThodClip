using System.Text;

namespace ThodClip.Worker;

internal static class Program
{
    public static async Task<int> Main()
    {
        Console.OutputEncoding = new UTF8Encoding(false);
        Console.InputEncoding = new UTF8Encoding(false);
        Console.Error.WriteLine($"ThodClip.Worker {typeof(Program).Assembly.GetName().Version} starting");

        var dataDir = Environment.GetEnvironmentVariable("THODHUB_DATA_DIR");
        if (string.IsNullOrWhiteSpace(dataDir))
            dataDir = Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ThodClip");
        dataDir = Path.GetFullPath(dataDir);
        try
        {
            Directory.CreateDirectory(dataDir);
        }
        catch (Exception ex)
        {
            Log.Error("cannot create data directory", ex);
            return 1;
        }

        await using var engine = new Engine(dataDir);
        engine.Start();
        var server = new RpcServer(engine);
        try
        {
            await server.RunAsync(Console.In, Console.Out, CancellationToken.None);
        }
        catch (Exception ex)
        {
            Log.Error("fatal", ex);
            return 1;
        }
        return 0;
    }
}

internal static class Log
{
    private static readonly object Gate = new();
    public static void Info(string message) { lock (Gate) Console.Error.WriteLine($"[info] {message}"); }
    public static void Warn(string message) { lock (Gate) Console.Error.WriteLine($"[warn] {message}"); }
    public static void Error(string message, Exception? ex = null)
    {
        lock (Gate) Console.Error.WriteLine($"[error] {message}" + (ex is null ? "" : ": " + ex.Message));
    }
}
