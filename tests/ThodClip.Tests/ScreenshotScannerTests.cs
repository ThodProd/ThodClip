using ThodClip.Core;

namespace ThodClip.Tests;

public sealed class ScreenshotScannerTests
{
    [Fact]
    public void Import_archives_new_images_and_skips_duplicates()
    {
        using var root = new TestRoot();
        var source = Path.Combine(Path.GetTempPath(), "thodclip-src", Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(source);
        var file = Path.Combine(source, "screenshot-2026-01-01.png");
        File.WriteAllBytes(file, TestImages.Png(System.Drawing.Color.Goldenrod));

        var oldOnly = Environment.GetEnvironmentVariable("THODCLIP_SCREENSHOT_ONLY");
        var oldDirs = Environment.GetEnvironmentVariable("THODCLIP_SCREENSHOT_DIRS");
        try
        {
            Environment.SetEnvironmentVariable("THODCLIP_SCREENSHOT_ONLY", "1");
            Environment.SetEnvironmentVariable("THODCLIP_SCREENSHOT_DIRS", source);
            using var store = root.Open(new ClipSettings { AutoImportScreenshots = false });

            var first = ScreenshotScanner.Import(store, CancellationToken.None);
            Assert.Equal(1, first.Scanned);
            Assert.Equal(1, first.Imported);
            Assert.Equal(0, first.Failed);

            var second = ScreenshotScanner.Import(store, CancellationToken.None);
            Assert.Equal(1, second.Scanned);
            Assert.Equal(0, second.Imported);
            Assert.Equal(1, second.Skipped);
            Assert.Equal(1, store.GetStats().ShotCount);
        }
        finally
        {
            Environment.SetEnvironmentVariable("THODCLIP_SCREENSHOT_ONLY", oldOnly);
            Environment.SetEnvironmentVariable("THODCLIP_SCREENSHOT_DIRS", oldDirs);
            try { Directory.Delete(source, true); }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { }
        }
    }

    [Fact]
    public void Known_directories_can_be_restricted_for_tests()
    {
        using var root = new TestRoot();
        var source = Path.Combine(root.Root, "shots");
        Directory.CreateDirectory(source);
        var oldOnly = Environment.GetEnvironmentVariable("THODCLIP_SCREENSHOT_ONLY");
        var oldDirs = Environment.GetEnvironmentVariable("THODCLIP_SCREENSHOT_DIRS");
        try
        {
            Environment.SetEnvironmentVariable("THODCLIP_SCREENSHOT_ONLY", "1");
            Environment.SetEnvironmentVariable("THODCLIP_SCREENSHOT_DIRS", source);
            Assert.Equal([source], ScreenshotScanner.KnownDirectories());
        }
        finally
        {
            Environment.SetEnvironmentVariable("THODCLIP_SCREENSHOT_ONLY", oldOnly);
            Environment.SetEnvironmentVariable("THODCLIP_SCREENSHOT_DIRS", oldDirs);
        }
    }
}

public sealed class SettingsStoreTests
{
    [Fact]
    public void Settings_survive_a_roundtrip_and_are_sanitized()
    {
        using var root = new TestRoot();
        var settings = new ClipSettings
        {
            CaptureEnabled = false,
            HonorClipboardExclusions = false,
            AutoImportScreenshots = false,
            DedupeSeconds = 999,
            MaxClips = -5,
            MaxTextBytes = 1,
            MaxScreenshotBytes = -1
        };

        SettingsStore.Save(root.Root, settings);
        var loaded = SettingsStore.Load(root.Root);

        Assert.False(loaded.CaptureEnabled);
        Assert.False(loaded.HonorClipboardExclusions);
        Assert.False(loaded.AutoImportScreenshots);
        Assert.Equal(60, loaded.DedupeSeconds);
        Assert.Equal(0, loaded.MaxClips);
        Assert.Equal(64 * 1024, loaded.MaxTextBytes);
        Assert.Equal(0, loaded.MaxScreenshotBytes);
    }

    [Fact]
    public void Missing_or_broken_settings_fall_back_to_defaults()
    {
        using var root = new TestRoot();
        Assert.True(SettingsStore.Load(root.Root).CaptureEnabled);

        File.WriteAllText(Path.Combine(root.Root, SettingsStore.FileName), "{ broken");
        var loaded = SettingsStore.Load(root.Root);
        Assert.True(loaded.CaptureEnabled);
        Assert.Equal(3, loaded.DedupeSeconds);
    }
}
