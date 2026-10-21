// Small window helpers shared by the inspection scripts.
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public static class DraftWindows
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int X, Y; }

    public delegate bool EnumProc(IntPtr handle, IntPtr state);

    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc callback, IntPtr state);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr handle, out uint pid);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr handle, out RECT rect);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr handle, IntPtr dc, uint flags);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr handle);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr handle);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint x, uint y, uint data, IntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr handle, StringBuilder text, int max);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr handle, StringBuilder text, int max);

    public static List<IntPtr> TopLevel(uint wanted)
    {
        var found = new List<IntPtr>();
        EnumWindows((handle, state) =>
        {
            uint pid;
            GetWindowThreadProcessId(handle, out pid);
            if (pid == wanted) found.Add(handle);
            return true;
        }, IntPtr.Zero);
        return found;
    }

    // The main window is the titled GPUI window; the HUD is the untitled one.
    public static IntPtr MainWindow(uint pid)
    {
        foreach (var handle in TopLevel(pid))
        {
            var cls = new StringBuilder(256);
            GetClassNameW(handle, cls, 256);
            var title = new StringBuilder(256);
            GetWindowTextW(handle, title, 256);
            if (cls.ToString() == "Zed::Window" && title.ToString() != "") return handle;
        }
        return IntPtr.Zero;
    }

    public static IntPtr HudWindow(uint pid)
    {
        foreach (var handle in TopLevel(pid))
        {
            var cls = new StringBuilder(256);
            GetClassNameW(handle, cls, 256);
            var title = new StringBuilder(256);
            GetWindowTextW(handle, title, 256);
            if (cls.ToString() == "Zed::Window" && title.ToString() == "") return handle;
        }
        return IntPtr.Zero;
    }
}
