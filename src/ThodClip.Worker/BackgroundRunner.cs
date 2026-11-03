using ThodClip.Core;

namespace ThodClip.Worker;

/// <summary>
/// Runs a single capture worker without the hub. It exits when the user disables background mode
/// or when ThodHUB asks it to stop before an update or uninstall.
/// </summary>
internal static class BackgroundRunner
{
    public static async Task<int> RunAsync(string dataDir)
    {
        using var single = new Mutex(false, InstanceNames.BackgroundMutex(dataDir));
        var acquired = false;
        try
        {
            acquired = single.WaitOne(0);
        }
        catch (AbandonedMutexException)
        {
            // The previous background worker crashed; this process now owns the mutex.
            acquired = true;
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or IOException)
        {
            Log.Error("cannot open the background worker mutex", ex);
            return 1;
        }

        if (!acquired)
        {
            Log.Info("background worker is already running");
            return 0;
        }

        using var stop = new EventWaitHandle(false, EventResetMode.ManualReset, InstanceNames.StopEvent(dataDir));
        stop.Reset();

        try
        {
            Directory.CreateDirectory(Autostart.CacheDirectory);
            Log.ConfigureFile(Autostart.LogPath);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Warn("cannot prepare the background log: " + ex.Message);
        }

        var settings = SettingsStore.Load(dataDir);
        if (!settings.RunInBackground &&
            Environment.GetEnvironmentVariable("THODCLIP_BACKGROUND_FORCE") != "1")
        {
            Log.Info("background mode is disabled in settings; exiting");
            return 0;
        }

        Log.Info($"background worker {typeof(BackgroundRunner).Assembly.GetName().Version} starting");
        var exitCode = 0;
        try
        {
            await using var engine = new Engine(dataDir, background: true);
            engine.Start();

            var stamp = SettingsStamp(dataDir);
            while (true)
            {
                if (stop.WaitOne(TimeSpan.FromSeconds(2)))
                {
                    Log.Info("stop requested by ThodHUB");
                    break;
                }

                var current = SettingsStamp(dataDir);
                if (current == stamp) continue;
                stamp = current;

                var next = SettingsStore.Load(dataDir);
                if (!next.RunInBackground)
                {
                    Log.Info("background mode was disabled; stopping");
                    break;
                }

                engine.ReloadSettings(next);
                Log.Info("settings reloaded");
            }
        }
        catch (Exception ex)
        {
            Log.Error("background worker failed", ex);
            exitCode = 1;
        }
        Log.Info("background worker stopped");
        return exitCode;
    }

    private static (long Ticks, long Length) SettingsStamp(string dataDir)
    {
        try
        {
            var info = new FileInfo(Path.Combine(dataDir, SettingsStore.FileName));
            return info.Exists ? (info.LastWriteTimeUtc.Ticks, info.Length) : (0, 0);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            return (0, 0);
        }
    }
}
