using System.Text;

namespace ThodClip.Worker;

internal enum WorkerMode
{
    Rpc,
    Background,
    RegisterIntegrations,
    UninstallIntegrations
}

internal static class Program
{
    public static async Task<int> Main(string[] args)
    {
        string dataDir;
        WorkerMode mode;
        try
        {
            ApplyHubHome(args);
            dataDir = ResolveDataDir(args);
            mode = ResolveMode(args);
        }
        catch (Exception ex) when (ex is ArgumentException or NotSupportedException or PathTooLongException)
        {
            Log.Error("invalid arguments: " + ex.Message);
            return 2;
        }

        try
        {
            Directory.CreateDirectory(dataDir);
        }
        catch (Exception ex)
        {
            Log.Error("cannot create data directory", ex);
            return 1;
        }

        if (mode == WorkerMode.RegisterIntegrations)
        {
            try
            {
                Autostart.Synchronize(dataDir);
                Log.Info("ThodClip integrations are registered");
                return 0;
            }
            catch (Exception ex)
            {
                Log.Error("cannot register ThodClip integrations", ex);
                return 1;
            }
        }

        if (mode == WorkerMode.UninstallIntegrations)
        {
            try
            {
                Autostart.RemoveInstallation(dataDir);
                Log.Info("ThodClip integrations are removed");
                return 0;
            }
            catch (Exception ex)
            {
                Log.Error("cannot remove ThodClip integrations", ex);
                return 1;
            }
        }

        if (mode == WorkerMode.Background)
            return await BackgroundRunner.RunAsync(dataDir);

        TrySetConsoleEncoding();
        Console.Error.WriteLine($"ThodClip.Worker {typeof(Program).Assembly.GetName().Version} starting");

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

    private static WorkerMode ResolveMode(IReadOnlyList<string> args)
    {
        foreach (var arg in args)
        {
            if (arg.Equals("--register-integrations", StringComparison.OrdinalIgnoreCase))
                return WorkerMode.RegisterIntegrations;
            if (arg.Equals("--uninstall-integrations", StringComparison.OrdinalIgnoreCase))
                return WorkerMode.UninstallIntegrations;
            if (arg.Equals("--background", StringComparison.OrdinalIgnoreCase))
                return WorkerMode.Background;
        }
        return WorkerMode.Rpc;
    }

    /// <summary>
    /// The Run entry starts the worker directly, without the hub environment. Preserve a custom
    /// THODHUB_HOME so the background log and cache copy use the same root as the hub.
    /// </summary>
    private static void ApplyHubHome(IReadOnlyList<string> args)
    {
        for (var i = 0; i < args.Count; i++)
        {
            var arg = args[i];
            if (arg.Equals("--home", StringComparison.OrdinalIgnoreCase) ||
                arg.Equals("--thodhub-home", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 >= args.Count || string.IsNullOrWhiteSpace(args[i + 1]))
                    throw new ArgumentException("Не указан каталог ThodHUB после " + arg + ".");
                Environment.SetEnvironmentVariable("THODHUB_HOME", Path.GetFullPath(args[i + 1]));
                return;
            }

            const string prefix = "--home=";
            if (arg.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) && arg.Length > prefix.Length)
            {
                Environment.SetEnvironmentVariable("THODHUB_HOME", Path.GetFullPath(arg[prefix.Length..]));
                return;
            }
        }
    }

    private static string ResolveDataDir(IReadOnlyList<string> args)
    {
        for (var i = 0; i < args.Count; i++)
        {
            var arg = args[i];
            if (arg.Equals("--data", StringComparison.OrdinalIgnoreCase) ||
                arg.Equals("--thodhub-data", StringComparison.OrdinalIgnoreCase))
            {
                if (i + 1 >= args.Count || string.IsNullOrWhiteSpace(args[i + 1]))
                    throw new ArgumentException("Не указан каталог данных после " + arg + ".");
                return Path.GetFullPath(args[i + 1]);
            }

            const string prefix = "--data=";
            if (arg.StartsWith(prefix, StringComparison.OrdinalIgnoreCase) && arg.Length > prefix.Length)
                return Path.GetFullPath(arg[prefix.Length..]);
        }

        var configured = Environment.GetEnvironmentVariable("THODHUB_DATA_DIR");
        if (!string.IsNullOrWhiteSpace(configured)) return Path.GetFullPath(configured);

        return Path.Combine(
            Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData),
            "ThodClip");
    }

    private static void TrySetConsoleEncoding()
    {
        try
        {
            Console.OutputEncoding = new UTF8Encoding(false);
            Console.InputEncoding = new UTF8Encoding(false);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or PlatformNotSupportedException)
        {
            // A GUI subsystem process started at logon has no console handles.
        }
    }
}
