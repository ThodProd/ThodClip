using System.Diagnostics;
using Microsoft.Win32;
using ThodClip.Core;

namespace ThodClip.Worker;

/// <summary>
/// Keeps a stable copy of the worker outside the versioned package folder and registers it for
/// the current user. The copy lives in ThodHUB's cache (not in user data or a backup snapshot),
/// so updates can replace the installed version without trying to overwrite a running file.
/// </summary>
internal static class Autostart
{
    private const string RunKeyPath = @"Software\Microsoft\Windows\CurrentVersion\Run";
    private const string ValueName = "ThodClip";
    private const string WorkerFileName = "ThodClip.Worker.exe";

    public static string HubRoot
    {
        get
        {
            var configured = Environment.GetEnvironmentVariable("THODHUB_HOME");
            return string.IsNullOrWhiteSpace(configured)
                ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.LocalApplicationData), "ThodHUB")
                : Path.GetFullPath(configured);
        }
    }

    public static string CacheDirectory => Path.Combine(HubRoot, "Cache", "thodclip");
    public static string StableDirectory => Path.Combine(CacheDirectory, "worker");
    public static string StableExecutable => Path.Combine(StableDirectory, WorkerFileName);
    public static string LogPath => Path.Combine(CacheDirectory, "background.log");

    public static bool IsRegistered()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, false);
            return key?.GetValue(ValueName) is string value &&
                   value.Contains(WorkerFileName, StringComparison.OrdinalIgnoreCase);
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or IOException or System.Security.SecurityException)
        {
            return false;
        }
    }

    public static void Apply(bool enabled, string dataDir)
    {
        if (enabled)
        {
            EnsureStableWorker(dataDir);
            Register(dataDir);
            StartBackground(dataDir);
        }
        else
        {
            Unregister();
            SignalStop(dataDir, TimeSpan.FromSeconds(3));
        }
    }

    /// <summary>
    /// Called by the install/update hook. It refreshes the stable copy and the Run entry but does
    /// not start a persistent worker: the hub worker already captures while the hub is open.
    /// </summary>
    public static void Synchronize(string dataDir)
    {
        var settings = SettingsStore.Load(dataDir);
        if (settings.RunInBackground)
        {
            EnsureStableWorker(dataDir);
            Register(dataDir);
            return;
        }

        Unregister();
        SignalStop(dataDir, TimeSpan.FromSeconds(2));
    }

    public static bool IsBackgroundRunning(string dataDir)
    {
        Mutex? mutex = null;
        try
        {
            mutex = Mutex.OpenExisting(InstanceNames.BackgroundMutex(dataDir));
            if (!mutex.WaitOne(0)) return true;
            mutex.ReleaseMutex();
        }
        catch (AbandonedMutexException)
        {
            // AbandonedMutexException still hands ownership to this thread. Release it so a new
            // background worker started from this process can acquire the mutex.
            try { mutex?.ReleaseMutex(); }
            catch (Exception ex) when (ex is ApplicationException or ObjectDisposedException) { }
        }
        catch (Exception ex) when (ex is WaitHandleCannotBeOpenedException or UnauthorizedAccessException or IOException)
        {
        }
        finally
        {
            mutex?.Dispose();
        }

        // A freshly started worker may not have acquired the mutex yet. Treat the cached process
        // as running so a quick disable does not race with startup.
        return IsStableWorkerAlive();
    }

    public static bool SignalStop(string dataDir, TimeSpan timeout)
    {
        try
        {
            using (var stop = new EventWaitHandle(false, EventResetMode.ManualReset, InstanceNames.StopEvent(dataDir)))
            {
                stop.Set();
            }

            var stopped = WaitUntilStopped(dataDir, timeout);
            if (!stopped)
            {
                // The worker ignored the event (for example, it is stuck in a shell call). Stopping
                // only our cached copy lets an update or uninstall finish safely.
                KillStableWorker();
                stopped = WaitUntilStopped(dataDir, TimeSpan.FromSeconds(3));
            }

            ResetStopEvent(dataDir);
            return stopped && !IsBackgroundRunning(dataDir);
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or IOException or WaitHandleCannotBeOpenedException)
        {
            Log.Warn("cannot signal background worker: " + ex.Message);
            return false;
        }
    }

    public static void RemoveInstallation(string dataDir)
    {
        Unregister();
        SignalStop(dataDir, TimeSpan.FromSeconds(8));
        for (var attempt = 0; attempt < 3; attempt++)
        {
            try
            {
                if (!Directory.Exists(CacheDirectory)) return;
                Directory.Delete(CacheDirectory, true);
                return;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
            {
                if (attempt == 2) Log.Warn("cannot remove cached worker: " + ex.Message);
                else Thread.Sleep(250);
            }
        }
    }

    private static bool WaitUntilStopped(string dataDir, TimeSpan timeout)
    {
        var deadline = DateTime.UtcNow + timeout;
        while (DateTime.UtcNow < deadline)
        {
            if (!IsBackgroundRunning(dataDir)) return true;
            Thread.Sleep(100);
        }
        return !IsBackgroundRunning(dataDir);
    }

    private static bool IsStableWorkerAlive()
    {
        try
        {
            var expected = Path.GetFullPath(StableExecutable);
            foreach (var process in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(WorkerFileName)))
            {
                using (process)
                {
                    string? path;
                    try { path = process.MainModule?.FileName; }
                    catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException or NotSupportedException)
                    {
                        continue;
                    }
                    if (path is not null && string.Equals(Path.GetFullPath(path), expected, StringComparison.OrdinalIgnoreCase))
                        return true;
                }
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
        }
        return false;
    }

    private static void KillStableWorker()
    {
        try
        {
            var expected = Path.GetFullPath(StableExecutable);
            foreach (var process in Process.GetProcessesByName(Path.GetFileNameWithoutExtension(WorkerFileName)))
            {
                using (process)
                {
                    string? path;
                    try { path = process.MainModule?.FileName; }
                    catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException or NotSupportedException)
                    {
                        continue;
                    }
                    if (path is null || !string.Equals(Path.GetFullPath(path), expected, StringComparison.OrdinalIgnoreCase))
                        continue;
                    try
                    {
                        process.Kill(entireProcessTree: true);
                        process.WaitForExit(3000);
                    }
                    catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException or NotSupportedException)
                    {
                        Log.Warn("cannot stop cached worker process: " + ex.Message);
                    }
                }
            }
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
        {
            Log.Warn("cannot inspect cached worker processes: " + ex.Message);
        }
    }

    private static void Register(string dataDir)
    {
        using var key = Registry.CurrentUser.CreateSubKey(RunKeyPath, true)
            ?? throw new InvalidOperationException("Недоступен раздел автозапуска Windows.");
        var command = $"\"{StableExecutable}\" --background --home \"{HubRoot}\" --data \"{dataDir}\"";
        key.SetValue(ValueName, command, RegistryValueKind.String);
    }

    private static void Unregister()
    {
        try
        {
            using var key = Registry.CurrentUser.OpenSubKey(RunKeyPath, true);
            if (key?.GetValue(ValueName) is string value &&
                value.Contains(WorkerFileName, StringComparison.OrdinalIgnoreCase))
                key.DeleteValue(ValueName, false);
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or IOException or System.Security.SecurityException)
        {
            Log.Warn("cannot remove autostart entry: " + ex.Message);
        }
    }

    private static void StartBackground(string dataDir)
    {
        if (IsBackgroundRunning(dataDir)) return;
        if (!File.Exists(StableExecutable))
            throw new FileNotFoundException("Файл фоновой записи не найден.", StableExecutable);

        ResetStopEvent(dataDir);
        var info = new ProcessStartInfo(StableExecutable)
        {
            WorkingDirectory = StableDirectory,
            UseShellExecute = false,
            CreateNoWindow = true
        };
        info.ArgumentList.Add("--background");
        info.ArgumentList.Add("--home");
        info.ArgumentList.Add(HubRoot);
        info.ArgumentList.Add("--data");
        info.ArgumentList.Add(dataDir);
        info.Environment["THODHUB_HOME"] = HubRoot;
        info.Environment["THODHUB_DATA_DIR"] = dataDir;
        try
        {
            Process.Start(info)?.Dispose();
        }
        catch (Exception ex) when (ex is System.ComponentModel.Win32Exception or InvalidOperationException)
        {
            throw new InvalidOperationException("Не удалось запустить фоновую запись: " + ex.Message, ex);
        }

        var deadline = DateTime.UtcNow + TimeSpan.FromSeconds(6);
        while (DateTime.UtcNow < deadline)
        {
            if (IsBackgroundRunning(dataDir)) return;
            Thread.Sleep(100);
        }
        throw new InvalidOperationException(
            "Фоновый процесс не запустился. Журнал: " + LogPath);
    }

    private static void ResetStopEvent(string dataDir)
    {
        try
        {
            using var stop = new EventWaitHandle(false, EventResetMode.ManualReset, InstanceNames.StopEvent(dataDir));
            stop.Reset();
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or IOException)
        {
            Log.Warn("cannot reset background stop event: " + ex.Message);
        }
    }

    private static void EnsureStableWorker(string dataDir)
    {
        var sourceDirectory = Path.GetFullPath(AppContext.BaseDirectory).TrimEnd(Path.DirectorySeparatorChar);
        var sourceExecutable = Environment.ProcessPath;
        if (string.IsNullOrWhiteSpace(sourceExecutable) || !File.Exists(sourceExecutable))
            throw new FileNotFoundException("Не найден исполняемый файл ThodClip.Worker.exe.");

        if (string.Equals(sourceDirectory, Path.GetFullPath(StableDirectory).TrimEnd(Path.DirectorySeparatorChar),
                StringComparison.OrdinalIgnoreCase))
            return;

        var sourceVersion = FileVersionInfo.GetVersionInfo(sourceExecutable).FileVersion ?? "";
        var stableVersion = File.Exists(StableExecutable)
            ? FileVersionInfo.GetVersionInfo(StableExecutable).FileVersion ?? ""
            : "";
        if (File.Exists(StableExecutable) && string.Equals(sourceVersion, stableVersion, StringComparison.Ordinal))
            return;

        if (!SignalStop(dataDir, TimeSpan.FromSeconds(8)))
            throw new IOException("Фоновая запись не остановилась. Перезапустите компьютер и повторите.");
        CopyDirectory(sourceDirectory, StableDirectory);
    }

    private static void CopyDirectory(string source, string destination)
    {
        var parent = Path.GetDirectoryName(destination)
            ?? throw new InvalidOperationException("Не найден каталог для фоновой копии.");
        Directory.CreateDirectory(parent);

        var temporary = destination + ".new-" + Guid.NewGuid().ToString("N");
        try
        {
            Directory.CreateDirectory(temporary);
            foreach (var file in Directory.EnumerateFiles(source, "*", SearchOption.AllDirectories))
            {
                var relative = Path.GetRelativePath(source, file);
                if (relative.EndsWith(".pdb", StringComparison.OrdinalIgnoreCase) ||
                    relative.EndsWith(".xml", StringComparison.OrdinalIgnoreCase) ||
                    relative.EndsWith(".log", StringComparison.OrdinalIgnoreCase))
                    continue;
                var target = Path.Combine(temporary, relative);
                Directory.CreateDirectory(Path.GetDirectoryName(target)!);
                File.Copy(file, target, true);
            }

            if (!File.Exists(Path.Combine(temporary, WorkerFileName)))
                throw new FileNotFoundException("В копии отсутствует ThodClip.Worker.exe.");

            string? previous = null;
            if (Directory.Exists(destination))
            {
                previous = destination + ".old-" + Guid.NewGuid().ToString("N");
                Directory.Move(destination, previous);
            }
            try
            {
                Directory.Move(temporary, destination);
            }
            catch
            {
                if (previous is not null && !Directory.Exists(destination)) Directory.Move(previous, destination);
                throw;
            }
            if (previous is not null)
            {
                try { Directory.Delete(previous, true); }
                catch (Exception ex) when (ex is IOException or UnauthorizedAccessException)
                {
                    Log.Warn("cannot delete the previous cached worker: " + ex.Message);
                }
            }
        }
        finally
        {
            try
            {
                if (Directory.Exists(temporary)) Directory.Delete(temporary, true);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { }
        }
    }
}
