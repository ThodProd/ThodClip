using System.Drawing;
using System.Drawing.Imaging;
using ThodClip.Core;

[assembly: CollectionBehavior(DisableTestParallelization = true)]

namespace ThodClip.Tests;

internal sealed class TestRoot : IDisposable
{
    public string Root { get; }

    public TestRoot()
    {
        Root = System.IO.Path.Combine(System.IO.Path.GetTempPath(), "thodclip-tests", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(Root);
    }

    public ClipStore Open(ClipSettings? settings = null) => new(Root, settings ?? new ClipSettings());

    public void Dispose()
    {
        try { Directory.Delete(Root, true); }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { }
    }
}

internal static class TestImages
{
    public static byte[] Png(Color color, int width = 48, int height = 32)
    {
        using var bitmap = new Bitmap(width, height);
        using (var graphics = Graphics.FromImage(bitmap)) graphics.Clear(color);
        using var stream = new MemoryStream();
        bitmap.Save(stream, ImageFormat.Png);
        return stream.ToArray();
    }
}

public sealed class ClipStoreTests
{
    [Fact]
    public void Text_is_stored_and_searchable()
    {
        using var root = new TestRoot();
        using var store = root.Open();
        var id = store.AddText("Привет, мир!", "notepad");

        Assert.True(id > 0);
        var item = store.GetClip(id);
        Assert.NotNull(item);
        Assert.Equal("text", item!.Kind);
        Assert.Equal("Привет, мир!", item.Text);
        Assert.Equal(1, item.RepeatCount);
        Assert.Equal("notepad", item.Source);

        var page = store.QueryClips("привет", null, false, null, 0, 10);
        Assert.Equal(1, page.Total);
        Assert.Single(page.Items);
    }

    [Fact]
    public void Consecutive_identical_copies_are_deduplicated()
    {
        using var root = new TestRoot();
        using var store = root.Open(new ClipSettings { DedupeSeconds = 3 });
        var first = store.AddText("same", null);
        var second = store.AddText("same", null);

        Assert.Equal(first, second);
        Assert.Equal(1, store.GetStats().ClipCount);
        Assert.Equal(2, store.GetClip(first)!.RepeatCount);
    }

    [Fact]
    public void Deduplication_can_be_disabled()
    {
        using var root = new TestRoot();
        using var store = root.Open(new ClipSettings { DedupeSeconds = 0 });
        store.AddText("same", null);
        store.AddText("same", null);
        Assert.Equal(2, store.GetStats().ClipCount);
    }

    [Fact]
    public void Pin_delete_and_clear_work()
    {
        using var root = new TestRoot();
        using var store = root.Open();
        var keep = store.AddText("keep", null);
        var remove = store.AddText("remove", null);
        store.AddText("third", null);

        Assert.True(store.PinClip(keep, true));
        Assert.Equal(2, store.ClearClips(keepPinned: true));
        Assert.NotNull(store.GetClip(keep));
        Assert.Null(store.GetClip(remove));
        Assert.Equal(1, store.GetStats().ClipCount);

        Assert.True(store.PinClip(keep, false));
        Assert.Equal(1, store.ClearClips(keepPinned: true));
        Assert.Equal(0, store.GetStats().ClipCount);
    }

    [Fact]
    public void Delete_removes_clip_files()
    {
        using var root = new TestRoot();
        using var store = root.Open();
        var id = store.AddImage(TestImages.Png(Color.CornflowerBlue), 48, 32, "test");
        var item = store.GetClip(id)!;
        var blob = store.ResolveBlob(item);
        var thumb = store.ThumbnailFor(item);
        Assert.True(File.Exists(blob));
        Assert.NotNull(thumb);
        Assert.True(File.Exists(thumb));

        Assert.Equal(1, store.DeleteClips([id]));
        Assert.False(File.Exists(blob));
        Assert.False(File.Exists(thumb));
        Assert.Null(store.GetClip(id));
    }

    [Fact]
    public void Retention_keeps_pinned_items()
    {
        using var root = new TestRoot();
        var settings = new ClipSettings { DedupeSeconds = 0, MaxClips = 3 };
        using var store = root.Open(settings);
        var pinned = store.AddText("pinned", null);
        store.PinClip(pinned, true);
        for (var i = 0; i < 6; i++) store.AddText("item " + i, null);

        Assert.Equal(3, store.GetStats().ClipCount);
        Assert.NotNull(store.GetClip(pinned));

        settings.MaxClips = 2;
        settings.DedupeSeconds = 0;
        store.PruneNow();
        Assert.True(store.GetStats().ClipCount <= 3);
        Assert.NotNull(store.GetClip(pinned));
    }

    [Fact]
    public void File_list_is_stored_as_one_entry()
    {
        using var root = new TestRoot();
        using var store = root.Open();
        var a = Path.Combine(root.Root, "a.txt");
        var b = Path.Combine(root.Root, "b.txt");
        File.WriteAllText(a, "a");
        File.WriteAllText(b, "b");
        var id = store.AddFiles([a, b, a], "explorer");
        var item = store.GetClip(id)!;
        Assert.Equal("files", item.Kind);
        Assert.Equal(2, item.Files.Count);
        Assert.Contains(a, item.Text);
    }

    [Fact]
    public void Large_texts_are_clipped_in_lists_but_full_in_details()
    {
        using var root = new TestRoot();
        using var store = root.Open();
        var text = new string('x', 9000);
        var id = store.AddText(text, null);

        var page = store.QueryClips(null, null, false, null, 0, 10);
        Assert.True(page.Items[0].TextClipped);
        Assert.Equal(4000, page.Items[0].Text.Length);
        Assert.Equal(text, store.GetClip(id)!.Text);
    }

    [Fact]
    public void Calendar_counts_days_and_day_filter_returns_only_that_day()
    {
        using var root = new TestRoot();
        using var store = root.Open();
        store.AddText("first", null);
        store.AddText("second", null);
        var today = DateTime.Today;
        var month = today.ToString("yyyy-MM");
        var day = today.ToString("yyyy-MM-dd");

        var calendar = store.QueryCalendar(month);
        Assert.Equal(2, calendar.Total);
        Assert.Equal(day, calendar.FirstDate);
        Assert.Equal(day, calendar.LastDate);
        Assert.Contains(calendar.Days, entry => entry.Date == day && entry.Count == 2);

        var page = store.QueryClips(null, null, false, day, 0, 10);
        Assert.Equal(2, page.Total);
        Assert.Equal(day, page.Date);
        Assert.Empty(store.QueryClips(null, null, false, "2000-01-01", 0, 10).Items);
    }

    [Fact]
    public void Screenshots_are_archived_deduplicated_and_removed()
    {
        using var root = new TestRoot();
        using var store = root.Open();
        var png = TestImages.Png(Color.SeaGreen);
        var first = store.AddScreenshotPng(png, "Snipping Tool");
        var second = store.AddScreenshotPng(png, "Snipping Tool");

        Assert.NotNull(first);
        Assert.Equal(first, second);
        var page = store.QueryShots(null, 0, 20);
        Assert.Equal(1, page.Total);
        var shot = page.Items[0];
        Assert.Equal("Snipping Tool", shot.Source);
        Assert.Equal(48, shot.Width);
        Assert.True(File.Exists(store.ResolveShot(shot)));
        Assert.True(File.Exists(store.ResolveShotThumb(shot)));

        Assert.Equal(1, store.DeleteShots([shot.Id]));
        Assert.Equal(0, store.GetStats().ShotCount);
        Assert.False(File.Exists(store.ResolveShot(shot)));
    }

    [Fact]
    public void Screenshot_quota_removes_oldest_items()
    {
        using var root = new TestRoot();
        var settings = new ClipSettings { MaxScreenshotBytes = 1 };
        using var store = root.Open(settings);
        store.AddScreenshotPng(TestImages.Png(Color.Red), null);
        Assert.Equal(0, store.GetStats().ShotCount);
    }
}
