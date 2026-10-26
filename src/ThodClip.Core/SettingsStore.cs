using System.Text.Json;

namespace ThodClip.Core;

public static class SettingsStore
{
    public const string FileName = "config.json";

    private static readonly JsonSerializerOptions Options = new(JsonSerializerDefaults.Web) { WriteIndented = true };

    public static ClipSettings Load(string root)
    {
        var path = Path.Combine(root, FileName);
        try
        {
            if (File.Exists(path))
            {
                var settings = JsonSerializer.Deserialize<ClipSettings>(File.ReadAllText(path), Options);
                if (settings is not null)
                {
                    settings.Sanitize();
                    return settings;
                }
            }
        }
        catch (Exception ex) when (ex is IOException or JsonException or UnauthorizedAccessException) { }
        return new ClipSettings();
    }

    public static void Save(string root, ClipSettings settings)
    {
        settings.Sanitize();
        Directory.CreateDirectory(root);
        var path = Path.Combine(root, FileName);
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllText(temporary, JsonSerializer.Serialize(settings, Options), new System.Text.UTF8Encoding(false));
            File.Move(temporary, path, true);
        }
        finally
        {
            if (File.Exists(temporary)) File.Delete(temporary);
        }
    }
}
