using System.Collections.Concurrent;
using System.Collections.Specialized;
using System.Diagnostics;
using System.Drawing;
using System.Runtime.InteropServices;
using System.Windows.Forms;
using ThodClip.Core;

namespace ThodClip.Worker;

internal sealed class ClipboardPayload
{
    public string Kind { get; init; } = "text";
    public string Text { get; init; } = "";
    public string[] Files { get; init; } = Array.Empty<string>();
    public byte[]? Png { get; init; }
    public int Width { get; init; }
    public int Height { get; init; }
    public string? Source { get; init; }
}

/// <summary>STA clipboard listener with a message-only window and WM_CLIPBOARDUPDATE.</summary>
internal sealed class ClipboardWatcher : IDisposable
{
    private const int WM_CLIPBOARDUPDATE = 0x031D;
    private const int WM_APP_INVOKE = 0x8000 + 1;
    private const int WM_QUIT = 0x0012;
    private const int HWND_MESSAGE = -3;

    private readonly ConcurrentQueue<(Action Action, TaskCompletionSource Completion)> _work = new();
    private readonly ManualResetEventSlim _ready = new(false);
    private readonly bool _honorExclusions;
    private Thread? _thread;
    private IntPtr _window;
    private volatile bool _running;
    private long _suppressUntil;
    private NativeMethods.WndProc? _wndProc;

    public event Action<ClipboardPayload>? Captured;
    public bool IsRunning => _running;
    public bool CaptureEnabled { get; set; } = true;

    public ClipboardWatcher(bool honorExclusions) => _honorExclusions = honorExclusions;

    public void Start()
    {
        if (_thread is not null) return;
        _thread = new Thread(ThreadMain) { IsBackground = true, Name = "ThodClip clipboard" };
        _thread.SetApartmentState(ApartmentState.STA);
        _thread.Start();
        if (!_ready.Wait(5000)) Log.Warn("clipboard listener did not start in time");
    }

    private void ThreadMain()
    {
        var className = "ThodClipClipboard_" + Guid.NewGuid().ToString("N");
        var instance = NativeMethods.GetModuleHandle(null);
        _wndProc = WndProc;
        var windowClass = new NativeMethods.WNDCLASS
        {
            lpfnWndProc = Marshal.GetFunctionPointerForDelegate(_wndProc),
            hInstance = instance,
            lpszClassName = className
        };
        if (NativeMethods.RegisterClass(ref windowClass) == 0)
        {
            Log.Error("RegisterClass failed");
            _ready.Set();
            return;
        }
        try
        {
            _window = NativeMethods.CreateWindowEx(0, className, "ThodClip", 0, 0, 0, 0, 0,
                new IntPtr(HWND_MESSAGE), IntPtr.Zero, instance, IntPtr.Zero);
            if (_window == IntPtr.Zero)
            {
                Log.Error("CreateWindowEx failed");
                return;
            }
            if (!NativeMethods.AddClipboardFormatListener(_window))
                Log.Warn("AddClipboardFormatListener failed");
            _running = true;
            _ready.Set();
            while (true)
            {
                var result = NativeMethods.GetMessage(out var message, IntPtr.Zero, 0, 0);
                if (result <= 0) break;
                NativeMethods.TranslateMessage(ref message);
                NativeMethods.DispatchMessage(ref message);
            }
        }
        catch (Exception ex)
        {
            Log.Error("clipboard message loop failed", ex);
        }
        finally
        {
            _running = false;
            if (_window != IntPtr.Zero)
            {
                NativeMethods.RemoveClipboardFormatListener(_window);
                NativeMethods.DestroyWindow(_window);
                _window = IntPtr.Zero;
            }
            NativeMethods.UnregisterClass(className, instance);
            while (_work.TryDequeue(out var item)) item.Completion.TrySetException(new InvalidOperationException("Буфер обмена остановлен."));
            _ready.Set();
        }
    }

    private IntPtr WndProc(IntPtr hwnd, uint message, IntPtr wParam, IntPtr lParam)
    {
        try
        {
            if (message == WM_CLIPBOARDUPDATE)
            {
                if (CaptureEnabled && DateTime.UtcNow.Ticks >= Interlocked.Read(ref _suppressUntil))
                    ReadClipboard();
            }
            else if (message == WM_APP_INVOKE)
            {
                while (_work.TryDequeue(out var item))
                {
                    try { item.Action(); item.Completion.TrySetResult(); }
                    catch (Exception ex) { item.Completion.TrySetException(ex); }
                }
            }
            else if (message == WM_QUIT)
            {
                NativeMethods.PostQuitMessage(0);
            }
        }
        catch (Exception ex)
        {
            Log.Error("clipboard event failed", ex);
        }
        return NativeMethods.DefWindowProc(hwnd, message, wParam, lParam);
    }

    private static bool IsExcluded(IDataObject data)
    {
        try
        {
            if (data.GetDataPresent("ExcludeClipboardContentFromMonitorProcessing")) return true;
            foreach (var format in new[] { "CanIncludeInClipboardHistory", "CanUploadToCloudClipboard" })
            {
                if (!data.GetDataPresent(format)) continue;
                var value = data.GetData(format);
                var number = ReadInt32(value);
                if (number == 0) return true;
            }
        }
        catch (Exception ex) when (ex is ExternalException or COMException or InvalidOperationException) { }
        return false;
    }

    private static int? ReadInt32(object? value)
    {
        try
        {
            if (value is int number) return number;
            if (value is uint unsigned) return unchecked((int)unsigned);
            if (value is byte[] bytes && bytes.Length >= 4) return BitConverter.ToInt32(bytes, 0);
            if (value is MemoryStream stream)
            {
                var buffer = new byte[4];
                var read = stream.Read(buffer, 0, 4);
                return read == 4 ? BitConverter.ToInt32(buffer, 0) : null;
            }
            if (value is Stream other)
            {
                var buffer = new byte[4];
                var read = other.Read(buffer, 0, 4);
                return read == 4 ? BitConverter.ToInt32(buffer, 0) : null;
            }
        }
        catch (Exception ex) when (ex is IOException or ObjectDisposedException) { }
        return null;
    }

    private void ReadClipboard()
    {
        for (var attempt = 0; attempt < 6 && _running; attempt++)
        {
            try
            {
                var data = Clipboard.GetDataObject();
                if (data is null) { Thread.Sleep(50); continue; }
                if (_honorExclusions && IsExcluded(data)) return;
                var source = SourceProcess();
                if (TryImageFile(data, source, out var imageFile)) { Publish(imageFile); return; }
                if (TryFiles(data, source, out var files)) { Publish(files); return; }
                if (TryImage(data, source, out var image)) { Publish(image); return; }
                if (TryText(data, source, out var text)) { Publish(text); return; }
                return;
            }
            catch (Exception ex) when (ex is ExternalException or COMException or InvalidOperationException)
            {
                Thread.Sleep(60);
            }
        }
    }

    private static bool TryFiles(IDataObject data, string? source, out ClipboardPayload payload)
    {
        payload = null!;
        if (!data.GetDataPresent(DataFormats.FileDrop)) return false;
        if (data.GetData(DataFormats.FileDrop) is not string[] files || files.Length == 0) return false;
        payload = new ClipboardPayload { Kind = "files", Files = files, Source = source };
        return true;
    }

    /// <summary>
    /// Some programs put a copied picture on the clipboard as a single file with a temporary
    /// name such as {GUID}.png. Keep it as a picture so the history shows a real preview.
    /// </summary>
    private static bool TryImageFile(IDataObject data, string? source, out ClipboardPayload payload)
    {
        payload = null!;
        try
        {
            if (!data.GetDataPresent(DataFormats.FileDrop)) return false;
            if (data.GetData(DataFormats.FileDrop) is not string[] files || files.Length != 1) return false;
            var path = files[0];
            if (string.IsNullOrWhiteSpace(path) || !ImageFiles.IsImagePath(path) || !File.Exists(path)) return false;
            var png = ImageFiles.TryEncodeFileAsPng(path, out var width, out var height);
            if (png is null) return false;
            payload = new ClipboardPayload
            {
                Kind = "image", Png = png, Width = width, Height = height, Source = source,
                Files = new[] { path }
            };
            return true;
        }
        catch (Exception ex) when (ex is ExternalException or COMException or ArgumentException or OutOfMemoryException) { }
        return false;
    }

    private static bool TryImage(IDataObject data, string? source, out ClipboardPayload payload)
    {
        payload = null!;
        try
        {
            if (data.GetDataPresent("PNG"))
            {
                var raw = data.GetData("PNG");
                byte[]? bytes = raw switch
                {
                    MemoryStream stream => ReadAll(stream),
                    byte[] array => array,
                    _ => null
                };
                if (bytes is { Length: > 8 } && bytes[0] == 0x89 && bytes[1] == 0x50 && bytes[2] == 0x4E && bytes[3] == 0x47)
                {
                    using var stream = new MemoryStream(bytes);
                    using var image = Image.FromStream(stream);
                    payload = new ClipboardPayload
                    {
                        Kind = "image", Png = bytes, Width = image.Width, Height = image.Height, Source = source
                    };
                    return true;
                }
            }
            if (data.GetDataPresent(DataFormats.Bitmap))
            {
                using var image = Clipboard.GetImage();
                if (image is null) return false;
                var bytes = ImageFiles.ToPng(image);
                payload = new ClipboardPayload
                {
                    Kind = "image", Png = bytes, Width = image.Width, Height = image.Height, Source = source
                };
                return true;
            }
        }
        catch (Exception ex) when (ex is ExternalException or COMException or ArgumentException or OutOfMemoryException) { }
        return false;
    }

    private static byte[] ReadAll(MemoryStream stream)
    {
        var position = stream.Position;
        stream.Position = 0;
        using var copy = new MemoryStream();
        stream.CopyTo(copy);
        stream.Position = position;
        return copy.ToArray();
    }

    private static bool TryText(IDataObject data, string? source, out ClipboardPayload payload)
    {
        payload = null!;
        try
        {
            string? text = null;
            if (data.GetDataPresent(DataFormats.UnicodeText))
                text = data.GetData(DataFormats.UnicodeText) as string;
            if (string.IsNullOrEmpty(text) && data.GetDataPresent(DataFormats.Text))
                text = data.GetData(DataFormats.Text) as string;
            if (string.IsNullOrWhiteSpace(text)) return false;
            payload = new ClipboardPayload { Kind = "text", Text = text, Source = source };
            return true;
        }
        catch (Exception ex) when (ex is ExternalException or COMException) { return false; }
    }

    private static string? SourceProcess()
    {
        try
        {
            var window = NativeMethods.GetClipboardOwner();
            if (window == IntPtr.Zero) window = NativeMethods.GetForegroundWindow();
            if (window == IntPtr.Zero) return null;
            NativeMethods.GetWindowThreadProcessId(window, out var pid);
            if (pid == 0) return null;
            using var process = Process.GetProcessById((int)pid);
            return process.ProcessName;
        }
        catch (Exception ex) when (ex is ArgumentException or InvalidOperationException or System.ComponentModel.Win32Exception) { return null; }
    }

    private void Publish(ClipboardPayload payload)
    {
        ThreadPool.QueueUserWorkItem(_ =>
        {
            try { Captured?.Invoke(payload); }
            catch (Exception ex) { Log.Error("clipboard capture failed", ex); }
        });
    }

    private Task Invoke(Action action)
    {
        if (!_running || _window == IntPtr.Zero) return Task.FromException(new InvalidOperationException("Служба буфера обмена не запущена."));
        var completion = new TaskCompletionSource(TaskCreationOptions.RunContinuationsAsynchronously);
        _work.Enqueue((action, completion));
        NativeMethods.PostMessage(_window, WM_APP_INVOKE, IntPtr.Zero, IntPtr.Zero);
        return completion.Task;
    }

    private void Suppress() => Interlocked.Exchange(ref _suppressUntil, DateTime.UtcNow.AddMilliseconds(1600).Ticks);

    public Task CopyText(string text) => Invoke(() =>
    {
        Suppress();
        Clipboard.SetDataObject(new DataObject(DataFormats.UnicodeText, text), true);
    });

    public Task CopyFiles(IReadOnlyList<string> files) => Invoke(() =>
    {
        var collection = new StringCollection();
        collection.AddRange(files.ToArray());
        Suppress();
        var data = new DataObject();
        data.SetData(DataFormats.FileDrop, collection);
        Clipboard.SetDataObject(data, true);
    });

    public Task CopyPng(byte[] png) => Invoke(() =>
    {
        Suppress();
        var data = new DataObject();
        using var pngStream = new MemoryStream(png);
        data.SetData("PNG", false, pngStream);
        using var imageStream = new MemoryStream(png);
        using var image = Image.FromStream(imageStream);
        data.SetData(DataFormats.Bitmap, true, image);
        Clipboard.SetDataObject(data, true);
    });

    public Task CopyFileImage(string path) => Invoke(() =>
    {
        byte[] png;
        using (var stream = new FileStream(path, FileMode.Open, FileAccess.Read, FileShare.ReadWrite | FileShare.Delete))
        using (var image = Image.FromStream(stream))
        {
            png = ImageFiles.ToPng(image);
        }
        Suppress();
        var data = new DataObject();
        using var pngStream = new MemoryStream(png);
        data.SetData("PNG", false, pngStream);
        using var imageStream = new MemoryStream(png);
        using var bitmap = Image.FromStream(imageStream);
        data.SetData(DataFormats.Bitmap, true, bitmap);
        Clipboard.SetDataObject(data, true);
    });

    public void Stop()
    {
        if (_thread is null) return;
        if (_window != IntPtr.Zero) NativeMethods.PostMessage(_window, WM_QUIT, IntPtr.Zero, IntPtr.Zero);
        _thread.Join(2500);
        _thread = null;
    }

    public void Dispose() => Stop();
}

internal static partial class NativeMethods
{
    internal delegate IntPtr WndProc(IntPtr hWnd, uint message, IntPtr wParam, IntPtr lParam);

    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    internal struct WNDCLASS
    {
        public uint style;
        public IntPtr lpfnWndProc;
        public int cbClsExtra;
        public int cbWndExtra;
        public IntPtr hInstance;
        public IntPtr hIcon;
        public IntPtr hCursor;
        public IntPtr hbrBackground;
        [MarshalAs(UnmanagedType.LPWStr)] public string? lpszMenuName;
        [MarshalAs(UnmanagedType.LPWStr)] public string lpszClassName;
    }

    [StructLayout(LayoutKind.Sequential)]
    internal struct MSG
    {
        public IntPtr hwnd;
        public uint message;
        public IntPtr wParam;
        public IntPtr lParam;
        public uint time;
        public int ptX;
        public int ptY;
    }

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    internal static extern ushort RegisterClass(ref WNDCLASS windowClass);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    internal static extern bool UnregisterClass(string className, IntPtr instance);

    [DllImport("user32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    internal static extern IntPtr CreateWindowEx(uint exStyle, string className, string windowName, uint style,
        int x, int y, int width, int height, IntPtr parent, IntPtr menu, IntPtr instance, IntPtr parameter);

    [DllImport("user32.dll", SetLastError = true)]
    internal static extern bool DestroyWindow(IntPtr window);

    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    internal static extern IntPtr DefWindowProc(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    internal static extern int GetMessage(out MSG message, IntPtr window, uint filterMin, uint filterMax);

    [DllImport("user32.dll")]
    internal static extern bool TranslateMessage(ref MSG message);

    [DllImport("user32.dll")]
    internal static extern IntPtr DispatchMessage(ref MSG message);

    [DllImport("user32.dll")]
    internal static extern bool PostMessage(IntPtr window, uint message, IntPtr wParam, IntPtr lParam);

    [DllImport("user32.dll")]
    internal static extern void PostQuitMessage(int exitCode);

    [DllImport("user32.dll", SetLastError = true)]
    internal static extern bool AddClipboardFormatListener(IntPtr window);

    [DllImport("user32.dll", SetLastError = true)]
    internal static extern bool RemoveClipboardFormatListener(IntPtr window);

    [DllImport("user32.dll")]
    internal static extern IntPtr GetClipboardOwner();

    [DllImport("user32.dll")]
    internal static extern IntPtr GetForegroundWindow();

    [DllImport("user32.dll")]
    internal static extern uint GetWindowThreadProcessId(IntPtr window, out uint processId);

    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)]
    internal static extern IntPtr GetModuleHandle(string? moduleName);
}
