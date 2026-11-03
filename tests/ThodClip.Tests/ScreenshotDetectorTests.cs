using ThodClip.Core;

namespace ThodClip.Tests;

public sealed class ScreenshotDetectorTests
{
    private const string ScreenClipPath =
        @"C:\Users\Thod\AppData\Local\Packages\MicrosoftWindows.Client.Core_cw5n1h2txyewy\TempState\ScreenClip\{CE816741-6A0F-4B41-9B5E-6A8D7C3F2A11}.png";
    private const string ScreenSketchPath =
        @"C:\Users\Thod\AppData\Local\Packages\Microsoft.ScreenSketch_8wekyb3d8bbwe\TempState\ScreenClip\snip.png";

    [Fact]
    public void Partial_snipping_tool_capture_from_the_clipboard_is_a_screenshot()
    {
        // Real Snipping Tool capture: 1545x975 is below the 85% screen-area threshold, the owner
        // is svchost, and the only reliable signal is the temporary ScreenClip path.
        var result = ScreenshotDetector.IsLikelyScreenshot(
            [ScreenClipPath], "svchost", 1545, 975, 1920, 1080);

        Assert.True(result);
    }

    [Fact]
    public void Screen_clip_and_screen_sketch_folders_are_recognized_by_path()
    {
        Assert.True(ScreenshotScanner.IsScreenshotPath(ScreenClipPath));
        Assert.True(ScreenshotScanner.IsScreenshotPath(ScreenSketchPath));
        Assert.True(ScreenshotScanner.IsScreenshotPath(
            @"C:\Users\Thod\AppData\Local\Packages\Microsoft.SnippingTool_8wekyb3d8bbwe\TempState\capture.png"));
    }

    [Fact]
    public void Ordinary_small_image_with_a_neutral_owner_is_not_a_screenshot()
    {
        var result = ScreenshotDetector.IsLikelyScreenshot(
            [@"C:\Users\Thod\AppData\Local\Temp\photo.png"], "Code", 800, 600, 1920, 1080);

        Assert.False(result);
    }

    [Fact]
    public void Screenshot_owner_is_a_strong_hint()
    {
        Assert.True(ScreenshotDetector.IsLikelyScreenshot(
            null, "SnippingTool.exe", 640, 480, 1920, 1080));
        Assert.True(ScreenshotDetector.IsLikelyScreenshot(
            null, "ScreenSketch", 640, 480, 1920, 1080));
    }

    [Fact]
    public void Full_screen_or_almost_full_screen_image_is_a_screenshot()
    {
        Assert.True(ScreenshotDetector.IsLikelyScreenshot(
            null, null, 1920, 1080, 1920, 1080));
        Assert.True(ScreenshotDetector.IsLikelyScreenshot(
            null, null, 1800, 1000, 1920, 1080));
    }

    [Fact]
    public void Explicit_archive_all_flag_wins()
    {
        Assert.True(ScreenshotDetector.IsLikelyScreenshot(
            [@"C:\Users\Thod\Pictures\photo.png"], "Code", 100, 100, 1920, 1080, archiveAllImages: true));
    }

    [Fact]
    public void Screenshot_names_and_standard_folders_are_recognized()
    {
        Assert.True(ScreenshotScanner.IsScreenshotPath(@"C:\Users\Thod\Pictures\Screenshots\anything.png"));
        Assert.True(ScreenshotScanner.IsScreenshotPath(@"C:\Users\Thod\Pictures\screenshot-2026-01-01.png"));
        Assert.True(ScreenshotScanner.IsScreenshotPath(@"C:\Users\Thod\Desktop\Снимок экрана.png"));
        Assert.True(ScreenshotScanner.IsScreenshotPath(@"D:\work\screencap.jpg"));
    }

    [Fact]
    public void Neutral_image_paths_and_non_images_are_not_screenshots()
    {
        Assert.False(ScreenshotScanner.IsScreenshotPath(@"C:\Users\Thod\Pictures\holiday-photo.png"));
        Assert.False(ScreenshotScanner.IsScreenshotPath(@"C:\Users\Thod\Temp\notes.txt"));
        Assert.False(ScreenshotScanner.IsScreenshotPath(""));
    }
}
