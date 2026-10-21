// Render-inspection helper: captures a window and reports its structure as a
// text grid so the layout can be checked without eyes on the image.
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

public static class Inspect
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);

    static double Lum(Color c)
    {
        return (0.2126 * c.R + 0.7152 * c.G + 0.0722 * c.B) / 255.0;
    }

    public static void Run(string path, int cellW, int cellH)
    {
        Bitmap bitmap = new Bitmap(path);
        int w = bitmap.Width, h = bitmap.Height;
        Console.WriteLine("image " + w + "x" + h);

        Color corner = bitmap.GetPixel(2, 2);
        Console.WriteLine("corner background: #" + corner.R.ToString("X2") + corner.G.ToString("X2") + corner.B.ToString("X2"));

        int cols = w / cellW, rows = h / cellH;
        char[,] grid = new char[rows, cols];
        double minX = 1e9, maxX = -1, minY = 1e9, maxY = -1;
        int brightCells = 0;

        for (int gy = 0; gy < rows; gy++)
        {
            for (int gx = 0; gx < cols; gx++)
            {
                double sum = 0; int count = 0;
                double bestLum = -1; Color best = Color.Black; double amber = 0;
                int yEnd = Math.Min((gy + 1) * cellH, h);
                int xEnd = Math.Min((gx + 1) * cellW, w);
                for (int y = gy * cellH; y < yEnd; y++)
                {
                    for (int x = gx * cellW; x < xEnd; x++)
                    {
                        Color c = bitmap.GetPixel(x, y);
                        double lum = Lum(c);
                        sum += lum; count++;
                        if (lum > bestLum) { bestLum = lum; best = c; }
                        if (c.R > 120 && c.R > c.B + 40) amber = Math.Max(amber, lum);
                    }
                }
                double avg = sum / Math.Max(count, 1);
                double bestRel = Lum(best);

                char glyph;
                if (amber > 0.35) glyph = '@';
                else if (bestRel > 0.62) glyph = '#';
                else if (bestRel > 0.34) glyph = '*';
                else if (avg > 0.05) glyph = '.';
                else glyph = ' ';

                grid[gy, gx] = glyph;
                if (glyph != ' ')
                {
                    if (gx * cellW < minX) minX = gx * cellW;
                    if ((gx + 1) * cellW > maxX) maxX = (gx + 1) * cellW;
                    if (gy * cellH < minY) minY = gy * cellH;
                    if ((gy + 1) * cellH > maxY) maxY = (gy + 1) * cellH;
                }
                if (bestRel > 0.62) brightCells++;
            }
        }

        for (int gy = 0; gy < rows; gy++)
        {
            char[] line = new char[cols];
            for (int gx = 0; gx < cols; gx++) line[gx] = grid[gy, gx];
            Console.WriteLine((gy * cellH).ToString().PadLeft(4) + " |" + new string(line) + "|");
        }
        Console.WriteLine();
        Console.WriteLine("ink bounds: x " + minX + ".." + maxX + " of " + w + ", y " + minY + ".." + maxY + " of " + h);
        Console.WriteLine("text cells: " + brightCells);

        double bestContrast = 0; Color textColor = Color.Black, bgColor = Color.Black;
        for (int y = 0; y < h; y += 2)
        {
            for (int x = 0; x < w; x += 2)
            {
                Color c = bitmap.GetPixel(x, y);
                double lum = Lum(c);
                if (lum > 0.62)
                {
                    Color bg = bitmap.GetPixel(Math.Min(x + 3, w - 1), Math.Min(y + 3, h - 1));
                    double bgLum = Lum(bg);
                    double a = Math.Max(lum, bgLum), b = Math.Min(lum, bgLum);
                    double contrast = (a + 0.05) / (b + 0.05);
                    if (contrast > bestContrast) { bestContrast = contrast; textColor = c; bgColor = bg; }
                }
            }
        }
        Console.WriteLine("best text contrast: " + bestContrast.ToString("0.00") + ":1  text #" +
            textColor.R.ToString("X2") + textColor.G.ToString("X2") + textColor.B.ToString("X2") +
            " on #" + bgColor.R.ToString("X2") + bgColor.G.ToString("X2") + bgColor.B.ToString("X2"));

        int edgeInk = 0;
        for (int x = 0; x < w; x++)
        {
            Color top = bitmap.GetPixel(x, 1);
            if (Lum(top) > 0.5) edgeInk++;
        }
        Console.WriteLine("bright pixels on the top edge row: " + edgeInk);

        int leftInk = 0;
        for (int y = 0; y < h; y++)
        {
            Color left = bitmap.GetPixel(1, y);
            if (Lum(left) > 0.5) leftInk++;
        }
        Console.WriteLine("bright pixels on the left edge column: " + leftInk);
        bitmap.Dispose();
    }
}
