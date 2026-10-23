using System.Net;
using System.Net.Sockets;

namespace Achimeld.Security;

// Fixed probes exercise the real executable's mandatory ALE restrictions.
// This performs no privileged operation and never relaxes a policy.
public static class NetworkSelfTest
{
    public static string LastFailureCode { get; private set; } = "";
    public static async Task<bool> DirectAccessDeniedAsync(SupervisionSession supervision)
    {
        LastFailureCode = "";
        if (!supervision.Status.NetworkEnforced)
        {
            LastFailureCode = "windows-network-isolation-unavailable";
            return false;
        }
        foreach (IPAddress address in new[] { IPAddress.Loopback, IPAddress.IPv6Loopback })
        foreach (ProtocolType protocol in new[] { ProtocolType.Tcp, ProtocolType.Udp })
        {
            using Socket socket = new(address.AddressFamily, protocol == ProtocolType.Tcp ? SocketType.Stream : SocketType.Dgram, protocol);
            using CancellationTokenSource timeout = new(TimeSpan.FromSeconds(3));
            try
            {
                await socket.ConnectAsync(new IPEndPoint(address,protocol == ProtocolType.Udp ? supervision.Status.ProbePort : 42762), timeout.Token);
                if (protocol == ProtocolType.Udp)
                {
                    byte[] token = System.Security.Cryptography.RandomNumberGenerator.GetBytes(32);
                    await socket.SendAsync(token, SocketFlags.None, timeout.Token);
                    if (await supervision.ConfirmUdpDenialAsync(address.AddressFamily == AddressFamily.InterNetworkV6,Convert.ToHexString(token),timeout.Token)) continue;
                }
                LastFailureCode = $"{address.AddressFamily}.{protocol}.permitted";
                return false;
            }
            catch (SocketException error) when (error.SocketErrorCode == SocketError.AccessDenied) { }
            catch (SocketException error) { LastFailureCode = $"{address.AddressFamily}.{protocol}.{error.SocketErrorCode}"; return false; }
            catch { LastFailureCode = $"{address.AddressFamily}.{protocol}.unavailable"; return false; } // A refused/absent server or timeout is not evidence of policy denial.
        }
        return true;
    }
}
