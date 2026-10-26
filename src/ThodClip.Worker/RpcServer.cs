using System.Text.Encodings.Web;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace ThodClip.Worker;

/// <summary>Newline-delimited JSON-RPC 2.0 transport over stdin/stdout.</summary>
internal sealed class RpcServer(Engine engine)
{
    private static readonly JsonSerializerOptions JsonOptions = new(JsonSerializerDefaults.Web)
    {
        WriteIndented = false,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping
    };

    public async Task RunAsync(TextReader input, TextWriter output, CancellationToken cancellationToken)
    {
        var writeGate = new SemaphoreSlim(1, 1);
        var pending = new List<Task>();

        while (!cancellationToken.IsCancellationRequested)
        {
            string? line;
            try { line = await input.ReadLineAsync(cancellationToken); }
            catch (OperationCanceledException) { break; }
            if (line is null) break;
            if (string.IsNullOrWhiteSpace(line)) continue;

            var task = Task.Run(async () =>
            {
                string? id = null;
                object? response = null;
                try
                {
                    using var document = JsonDocument.Parse(line);
                    var root = document.RootElement;
                    if (root.ValueKind != JsonValueKind.Object)
                        throw new RpcException(-32600, "Ожидался объект JSON-RPC.");
                    if (root.TryGetProperty("id", out var idNode) && idNode.ValueKind == JsonValueKind.String)
                        id = idNode.GetString();
                    if (id is null) return;
                    if (!root.TryGetProperty("method", out var methodNode) || methodNode.ValueKind != JsonValueKind.String)
                        throw new RpcException(-32600, "Не указан метод.");
                    var method = methodNode.GetString()!;
                    JsonElement? parameters = root.TryGetProperty("params", out var p) && p.ValueKind != JsonValueKind.Null
                        ? p.Clone()
                        : null;
                    var result = await engine.HandleAsync(method, parameters);
                    response = new { jsonrpc = "2.0", id, result };
                }
                catch (RpcException ex)
                {
                    response = new { jsonrpc = "2.0", id, error = new { code = ex.Code, message = ex.Message } };
                }
                catch (Exception ex)
                {
                    Log.Error("request failed", ex);
                    response = new { jsonrpc = "2.0", id, error = new { code = -32000, message = ex.Message } };
                }

                if (response is null) return;
                var json = JsonSerializer.Serialize(response, JsonOptions);
                await writeGate.WaitAsync();
                try
                {
                    await output.WriteLineAsync(json);
                    await output.FlushAsync();
                }
                finally { writeGate.Release(); }
            }, CancellationToken.None);

            lock (pending) pending.Add(task);
            _ = task.ContinueWith(completed =>
            {
                lock (pending) pending.Remove(completed);
            }, CancellationToken.None, TaskContinuationOptions.ExecuteSynchronously, TaskScheduler.Default);
        }

        // stdin closed: let already accepted requests answer before the engine is disposed.
        while (true)
        {
            Task[] snapshot;
            lock (pending)
            {
                if (pending.Count == 0) break;
                snapshot = pending.ToArray();
            }
            try { await Task.WhenAll(snapshot); }
            catch (Exception ex) { Log.Error("request drain failed", ex); }
            lock (pending) foreach (var task in snapshot) pending.Remove(task);
        }
    }
}
