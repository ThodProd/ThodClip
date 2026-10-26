using System.Text.Json.Serialization;

namespace ThodClip.Core;

/// <summary>One entry of the clipboard history.</summary>
public sealed class ClipItem
{
    public long Id { get; init; }
    public string Kind { get; init; } = "text";
    public string Text { get; init; } = "";
    public string Preview { get; init; } = "";
    public IReadOnlyList<string> Files { get; init; } = Array.Empty<string>();
    public int Width { get; init; }
    public int Height { get; init; }
    public long Bytes { get; init; }
    public string? Source { get; init; }
    public string CreatedAt { get; init; } = "";
    public string UpdatedAt { get; init; } = "";
    public int RepeatCount { get; init; } = 1;
    public bool Pinned { get; init; }
    public bool Truncated { get; init; }
    public bool TextClipped { get; init; }
    public string? Thumb { get; set; }
    [JsonIgnore] public string? Blob { get; init; }
}

public sealed class ClipPage
{
    public long Revision { get; init; }
    public int Total { get; init; }
    public int Offset { get; init; }
    public int Limit { get; init; }
    public string? Date { get; init; }
    public IReadOnlyList<ClipItem> Items { get; init; } = Array.Empty<ClipItem>();
}

public sealed class CalendarDay
{
    public string Date { get; init; } = "";
    public int Count { get; init; }
}

public sealed class ClipCalendar
{
    public string Month { get; init; } = "";
    public int Total { get; init; }
    public string? FirstDate { get; init; }
    public string? LastDate { get; init; }
    public IReadOnlyList<CalendarDay> Days { get; init; } = Array.Empty<CalendarDay>();
}

public sealed class ShotItem
{
    public string Id { get; init; } = "";
    public string Name { get; init; } = "";
    public int Width { get; init; }
    public int Height { get; init; }
    public long Bytes { get; init; }
    public string Source { get; init; } = "";
    public string CreatedAt { get; init; } = "";
    public string? Thumb { get; set; }
    [JsonIgnore] public string RelativePath { get; init; } = "";
    [JsonIgnore] public string ThumbRelativePath { get; init; } = "";
    [JsonIgnore] public string Hash { get; init; } = "";
}

public sealed class ShotPage
{
    public long Revision { get; init; }
    public int Total { get; init; }
    public int Offset { get; init; }
    public int Limit { get; init; }
    public IReadOnlyList<ShotItem> Items { get; init; } = Array.Empty<ShotItem>();
}

public sealed class ClipSettings
{
    public bool CaptureEnabled { get; set; } = true;
    public bool HonorClipboardExclusions { get; set; } = true;
    public int DedupeSeconds { get; set; } = 3;
    public int MaxClips { get; set; }
    public int MaxTextBytes { get; set; } = 1024 * 1024;
    public long MaxScreenshotBytes { get; set; }
    public bool AutoImportScreenshots { get; set; } = true;

    public ClipSettings Clone() => (ClipSettings)MemberwiseClone();

    public void Sanitize()
    {
        DedupeSeconds = Math.Clamp(DedupeSeconds, 0, 60);
        MaxClips = Math.Clamp(MaxClips, 0, 200_000);
        MaxTextBytes = Math.Clamp(MaxTextBytes, 64 * 1024, 2 * 1024 * 1024);
        MaxScreenshotBytes = Math.Clamp(MaxScreenshotBytes, 0, 50L * 1024 * 1024 * 1024);
    }
}

public sealed class StorageStats
{
    public int ClipCount { get; init; }
    public int ShotCount { get; init; }
    public long ClipBytes { get; init; }
    public long ShotBytes { get; init; }
    public long DatabaseBytes { get; init; }
    public long TotalBytes => ClipBytes + ShotBytes + DatabaseBytes;
}

public sealed class ImportSummary
{
    public int Scanned { get; init; }
    public int Imported { get; init; }
    public int Skipped { get; init; }
    public int Failed { get; init; }
    public IReadOnlyList<string> Folders { get; init; } = Array.Empty<string>();
}
