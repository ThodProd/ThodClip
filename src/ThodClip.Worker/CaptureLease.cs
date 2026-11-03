namespace ThodClip.Worker;

/// <summary>
/// Only one process may read the clipboard and watch screenshot folders. The hub worker and the
/// background worker compete for this per-user mutex, so closing the hub hands capture over
/// without duplicating entries.
/// </summary>
internal sealed class CaptureLease : IDisposable
{
    private readonly Mutex _mutex;
    private bool _held;

    public CaptureLease(string name) => _mutex = new Mutex(false, name);

    public bool TryAcquire()
    {
        if (_held) return true;
        try
        {
            _held = _mutex.WaitOne(0);
        }
        catch (AbandonedMutexException)
        {
            // The previous owner exited without releasing: the mutex now belongs to us.
            _held = true;
        }
        catch (Exception ex) when (ex is ObjectDisposedException or UnauthorizedAccessException or IOException)
        {
            _held = false;
        }
        return _held;
    }

    public void Release()
    {
        if (!_held) return;
        _held = false;
        try { _mutex.ReleaseMutex(); }
        catch (Exception ex) when (ex is ApplicationException or ObjectDisposedException) { }
    }

    public void Dispose()
    {
        Release();
        _mutex.Dispose();
    }
}
