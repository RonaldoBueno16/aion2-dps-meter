using System.Net;
using System.Net.NetworkInformation;
using System.Net.Sockets;
using System.Runtime.Versioning;

namespace Aion2Meter.Core.Captura;

/// <summary>
/// Captura passiva com raw socket do Windows (SIO_RCVALL), sem driver extra. Exige o
/// processo elevado. Abre um socket por IPv4 local e entrega cada segmento TCP no
/// callback, serializado por lock (o pipeline não é thread-safe).
/// Não abre handle nem lê nada do processo do jogo.
/// </summary>
[SupportedOSPlatform("windows")]
public sealed class CapturaSocketBruto : IDisposable
{
    private const int EnlaceIpv4 = 228;

    private readonly Action<SegmentoTcp, DateTime> aoSegmento;
    private readonly Lock trava = new();
    private readonly List<Socket> sockets = [];
    private volatile bool parando;

    public IReadOnlyList<IPAddress> Enderecos { get; private set; } = [];
    public long Recebidos;
    public long SegmentosTcp;

    public CapturaSocketBruto(Action<SegmentoTcp, DateTime> aoSegmento) => this.aoSegmento = aoSegmento;

    public void Iniciar()
    {
        Enderecos = NetworkInterface.GetAllNetworkInterfaces()
            .Where(n => n.OperationalStatus == OperationalStatus.Up && n.NetworkInterfaceType != NetworkInterfaceType.Loopback)
            .SelectMany(n => n.GetIPProperties().UnicastAddresses)
            .Select(u => u.Address)
            .Where(a => a.AddressFamily == AddressFamily.InterNetwork)
            .ToList();
        if (Enderecos.Count == 0) throw new InvalidOperationException("Nenhuma interface IPv4 ativa.");

        foreach (var endereco in Enderecos)
        {
            var s = new Socket(AddressFamily.InterNetwork, SocketType.Raw, ProtocolType.IP);
            try
            {
                s.Bind(new IPEndPoint(endereco, 0));
                s.SetSocketOption(SocketOptionLevel.IP, SocketOptionName.HeaderIncluded, true);
                s.ReceiveBufferSize = 8 * 1024 * 1024;
                s.IOControl(IOControlCode.ReceiveAll, BitConverter.GetBytes(1), new byte[4]);
            }
            catch (SocketException ex) when (ex.SocketErrorCode == SocketError.AccessDenied)
            {
                s.Dispose();
                throw new UnauthorizedAccessException("Captura por raw socket precisa rodar como administrador.", ex);
            }
            sockets.Add(s);
            new Thread(() => Receber(s)) { IsBackground = true, Name = $"captura {endereco}" }.Start();
        }
    }

    private void Receber(Socket s)
    {
        var buffer = new byte[65536];
        while (!parando)
        {
            int n;
            try { n = s.Receive(buffer); }
            catch (SocketException) when (parando) { return; }
            catch (ObjectDisposedException) { return; }

            Interlocked.Increment(ref Recebidos);
            if (!SegmentoTcp.TentarExtrair(buffer.AsSpan(0, n), EnlaceIpv4, out var seg)) continue;
            Interlocked.Increment(ref SegmentosTcp);
            var hora = DateTime.UtcNow;
            lock (trava) aoSegmento(seg, hora);
        }
    }

    public void Dispose()
    {
        parando = true;
        foreach (var s in sockets) s.Dispose();
        sockets.Clear();
    }
}
