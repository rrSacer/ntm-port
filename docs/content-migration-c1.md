# Migração C1: durabilidade por componentes

O conteúdo commitado declara a durabilidade dos **26 itens duráveis** por `"components": {"minecraft:max_damage": N}`. O campo `max_damage` na raiz da declaração não é mais emitido: o upstream atual o recusa. O catálogo `ids.json` conserva `max_damage` como dado de origem das máquinas; ele não é a declaração carregada pelo motor.

A geração agora emite o componente por padrão. A flag `--max-damage` continua aceita por compatibilidade com os comandos anteriores. `tools/build-visual.sh` conserva o mesmo formato ao empacotar e remove qualquer campo antigo residual. Matrizes eternas de desh e lâminas de desh não recebem o componente; os itens duráveis continuam com empilhamento 1.

**Um conteúdo-fonte e um pacote.** O mesmo `.hmod` é aceito pelo upstream atual e pelo fork com o patch `70d5b95aa`, no binário `horizon-patched` de 30/10/2026 às 10:28 com runtime de mods. A instalação antiga deve atualizar binário e mods juntos. Não há conversão para um pacote legado. Os templates de bombas ficam para C2/L4.

Foi reaproveitado o suporte oficial a componentes e o empacotamento existente, sem novo formato. Os testes existentes cobrem lâminas de aço/titânio/desh e matrizes, inclusive `stamp_357`, com o padrão novo e a flag antiga; proíbem `max_damage` solto.

## Verificação

- **162 testes Python por seed (1 e 42)** em exportação isolada, e os validadores de conteúdo e receitas passaram. As 26 declarações foram conferidas contra os limites de `ids.json`. A comparação com o conteúdo-base de 2.695 IDs encontrou diferenças somente nessas 26 declarações.
- **Check upstream e check fork passaram** no mesmo pacote: 6.342.200 bytes, SHA-256 `b5bf00131594c2cec40a67fa08d3baf145b2afb930a1058a1cf4f0c58cd9fe22`. Disponível em `dist/ntm-content-c1.hmod`; a cópia isolada usada pela V0 está em `.agent-tmp/content-c1/content/target/horizon-mod/ntm.hmod` no workspace pai.
- **Prova em jogo passou**, com `horizon-patched` 10:28, os dois mods ntmlogic, Xvfb, áudio desligado e mundo temporário: matriz de chapa **0→8**, oito chapas produzidas; matriz de fio com dano inicial 62 **quebrou após dois ciclos**, 16 fios e três lingotes restantes; lâmina de aço **0→8**, desh **0→0**, 20 pós de ferro e dois cascalhos. O detector não encontrou rejeição do conteúdo nem panic.
- Os roteiros existentes de prensa/triturador foram reaproveitados. A bateria foi configurada e salva antes de reabrir o mundo para a prova elétrica, evitando que a tela antiga de configuração se sobrepusesse ao menu de slots. Não mudou a lógica nem os tempos de processamento das máquinas.
- O agente de lógica verificou o HEAD conjunto em exportação isolada: workspace verde (**296 testes do ntm-core**) e clippy sem warnings. A C1 altera declarações/geração, sem regras Rust.

Evidências em `content-gen-evidence/migration-c1/`: `package.json`, `content-diff.json`, testes e validadores, checks dos dois motores, `game-acceptance.json`, `presses-evidence.json`, `shredder-evidence.json`, capturas e `client.log`. A exportação C1 não inclui o lote 4 de energia ainda em edição.
