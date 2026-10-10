using System.Text;
using Xdos.Abstractions;

// _AppName_ — userland app template. Syscalls execute only in a ring-3 Xdos image;
// on Windows, `dotnet run` checks that the shared API compiles.

Console.WriteLine("_AppName_: host build (syscalls inactive on Windows).");

var msg = Encoding.UTF8.GetBytes(ThemeTokens.ShellPrompt + "hello from C#\n");
try
{
    Syscall.Write(1, msg);
}
catch (PlatformNotSupportedException)
{
    Console.WriteLine("(expected on host — publish to xdos when loader is enabled)");
}
