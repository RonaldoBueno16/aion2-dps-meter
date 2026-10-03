## Como instalar

1. Baixe o `Aion2Meter-<versão>-win-x64.zip` abaixo e extraia numa pasta. Se o navegador avisar que o arquivo é pouco baixado ou perigoso, escolha manter (às vezes a opção fica no menu `⋯` do download): o aviso vem da falta de assinatura digital.
2. Abra o `Axon.exe` **antes de entrar no mundo** (assim ele lê o seu level e Power no login). Ele fica como ícone ao lado do relógio (na seta ^) e o overlay aparece quando o jogo está na frente.
3. Aceite o pedido de administrador: a leitura do tráfego do jogo exige. Na primeira abertura, o overlay pede para liberar o Axon no Firewall do Windows: clique em **Liberar** (cria a regra "Axon (captura)", só para o próprio exe). Sem ela, o firewall pode barrar o que chega do servidor do jogo.
4. Se o Windows mostrar "O Windows protegeu o computador", clique em "Mais informações" e "Executar assim mesmo" (o executável não tem assinatura digital).

Se aparecer "O Controle Inteligente de Aplicativos bloqueou" (Windows 11), não há como liberar só o Axon: ele passa nesse recurso quando o exe tiver assinatura. Para conferir que o arquivo é o desta release, rode `Get-FileHash .\Axon.exe` no PowerShell e compare com o SHA-256 mostrado ao lado do `Axon.exe` abaixo.

Se o rodapé do overlay disser que o firewall está barrando, libere o `Axon.exe` no firewall do seu antivírus (Kaspersky, Avast, Norton...). Se disser que o servidor não foi encontrado, feche VPN, ExitLag ou programa parecido e abra o Axon de novo.

Não precisa instalar nada: é um executável único de cerca de 2 MB. A partir da 0.5.0, quando sair uma versão nova, o rodapé do overlay mostra o botão Atualizar, que baixa, confere e reabre o Axon sozinho. Para atualizar à mão, feche pelo ✕ do overlay ou em "Fechar Axon" no ícone ao lado do relógio e troque o `.exe` pelo da versão nova. Desde a 0.3.0 o executável se chama `Axon.exe`: pode apagar o `Aion2Meter.exe` antigo (e o `Aion2Meter.Overlay.exe`, de quem vem da 0.1.0). Os dados salvos ficam em `%LOCALAPPDATA%\Aion2Meter` e continuam valendo de uma versão para outra.

O medidor só lê os pacotes que o servidor manda para o seu PC: não injeta código, não lê a memória do jogo, não envia nada ao servidor e não automatiza ações. Mesmo assim, nenhum medidor de DPS é aprovado pela NCSoft: o uso é por sua conta e risco.
