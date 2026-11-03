# Energia: RTG e radgen

## Runtime — 03/11/2026 (branch rtg-radgen)

As duas identidades agora usam `Kind::Radioisotope`. O núcleo está em
`crates/ntm-core/src/radioisotopes.rs`, com tabela gerada do contrato Java.
Telas, modelo e multibloco já declarados em `visual.json` são reaproveitados.
A tela RTG conserva calor/capacidade e HE; a do radgen expõe progresso, duração
e produção das doze filas. O motor não recebe setters novos nesta branch.

| Máquina | Contrato de execução |
| --- | --- |
| RTG | 15 slots manuais quaisquer, como Slot vanilla. A classe base Java recusa inserção por funil e o RTG recusa extração; os índices são expostos em todas as faces, mas **o RTG não tem automação por funil no Java**. Só pellets vivos geram calor, uma contribuição por stack. Buffer 100.000 HE, limite 600 TU e 5 HE/TU nos defaults. |
| Vida | `custom_data.PELLET_DEPLETION` é i64. Ausência do compound significa vida completa; compound sem campo significa zero. O calor é lido antes do decremento/substituição. Outros campos são preservados no decremento; o produto é um stack novo de um item, como no Java. |
| Radgen | Entradas 0..11 e saídas 12..23; 28 combustíveis, cada fila guarda progresso, duração, produção e item singular sem NBT. Consome no início; gera também no primeiro e último ticks; reinicia só no tick seguinte. Buffer cheio não para a reação. Ao quebrar, cai só o inventário; combustível já consumido não é reembolsado, como no Java. |
| Resíduos | A admissão verifica item/meta e capacidade da saída. A conclusão segue o Java: não revalida saída alterada por comandos. Sucata não deixa resíduo; gema deixa diamante. |
| Portas HE | RTG: `around`, seis faces no núcleo. Radgen: uma porta a três blocos atrás do núcleo, aberta só para trás, rotacionada com facing. Nenhuma das duas lê redstone para desligar. |
| Ordem de energia | A etapa transacional grava `pending` junto do consumo/vida/fila. A rede pode retirar apenas o buffer anterior. Depois soma-se a geração pendente e aplica-se o limite; assim não se perde geração pelo clamp antecipado e não se oferece energia produzida no mesmo tick. A pendência é persistida para recuperar o estágio confirmado. |
| Persistência | JSON no BE é publicado junto dos slots, sob guarda da geração. `pending`, filas e configuração de RTG são serializados; ao reabrir, slots do BE permanecem a fonte dos itens. Campos de tela são derivados. |
| Hazard | O extrator exporta `rtg_target_rad_s`. Servidor e tooltip interpolam a radiação pela vida usando o `custom_data` de `NativeCarried`. Endpoints operam em float32 e interpolação em double, conforme Java. |

Defaults Java: `doRTGsDecay=true`, `scaleRTGPower=false`, perfil 528 desligado.
O núcleo cobre os ramos de decaimento e escala; configuração global externa de
perfis ainda não faz parte do port. HOT/BLINDING continuam na tabela de conteúdo;
a implementação geral desses hazards não integra este recorte de geração/radiação.
As cinco reciclagens de pellets esgotados continuam exigindo o retorno da carcaça
já registrado no lote de conteúdo abaixo.

**Política e portas integradas — pin `7607685b8`:**

A política pura de slots é consultada na previsão e no servidor, com o
inventário atual e a origem `Manual` ou `Automation`. RTG recusa inserção e
extração automáticas; radgen equilibra as 12 entradas apenas na automação.
Manual aceita qualquer item nas entradas. Ambos usam `quick_move=None`: os
`transferStackInSlot` Java fazem merge de qualquer item nesse intervalo, e
somente as saídas do radgen impedem inserção. Não foi aplicado um filtro de
combustível ao shift-click.

As três portas de inventário do radgen resolvem o dummy até o núcleo:
`multiblock.inventory_ports` declara `[0,0,3]`, `[1,0,0]` e `[-1,0,0]` no quadro
do colocador. A rede HE do NTM usa apenas a porta traseira `[0,0,3]`, aberta em
`[0,0,1]`, também registrada em `energy_ports`. Uma célula estrutural não-porta
não expõe o contêiner. O núcleo permanece acessível ao funil **por baixo para
extração**. Acima e nas laterais ele está envolto por dummies: o Java não permite
inserção direta no núcleo por funil vanilla nessa geometria. Por decisão do
coordenador, a prova não perfura o multibloco nem usa uma forma sintética; a
inserção por porta e os testes da mesma política cobrem a regra.

A fase `automation` conta 24 gems entre entrada e processamento, depois 12
diamantes +12 gems após sprint natural, e conserva 24 após extrações pela porta
lateral e sob o núcleo. A cena `radioisotopes-u98qgbku` passou as **66 verificações** no pin `7607685b8`,
sem fase excluída. O funil lateral extraiu 5 diamantes e o funil sob o núcleo
extraiu 7; nenhum retirou gem. A célula não-porta manteve a pedra e o RTG manteve
sua recusa de inserção/extração automáticas.

Na reabertura, a vida radium i64 caiu de **11.519.957.675 para 11.519.957.479**,
as filas avançaram mantendo os mesmos itens em processamento e as baterias
receberam **14.210→41.615 HE** e **35.000→170.000 HE**. Teste nativo de
preservação do i64 e dos campos extras passou; Clippy nativo sem `-A` passou.
A suíte do pin novo tem **387 unitários +26 integração**, Clippy core limpo e
**219 Python**. Regressões completas no mesmo release estão em andamento.

Índice: `.agent-tmp/rtg/runtime-validation.json`.
Cliente SHA-256 `3f9402c7da8dbfc11ef06294871e9f0457f897037d3dd519fbabcc7fb4028c21`;
pacote SHA-256 `b18682da6df57763ecf5955ca9bdc276c4cb314661aebb3ee7af0f3ee9d8f06e`.

### Histórico da aceitação anterior, pin ec479adb3

Validação intermediária: 383 testes unitários Rust e 26 de integração passaram;
Clippy `--workspace --all-targets -- -D warnings` passou. A suíte inclui os 28
combustíveis até conclusão real, conservação de itens/energia, recusas de commit,
conflito de inventário, i64 acima de i32, interpolação, quatro orientações da porta
e retomada estrita. O teste nativo de ida/volta do i64 e dos campos extras também passou. Clippy do
cliente/bundle transformado passou com `-D warnings`, sem qualquer `-A`.

A prova manual `radioisotopes-jou6nkkx` passou 50 verificações no pin `ec479adb3`
(upstream congelado `fc0210381`): montagem, 12 filas manuais, entrada de não
combustível, recusa de funil RTG, geração nas duas baterias, chumbo→bismuto,
resíduo tiny, sucata sem resíduo, gema→diamante e extração pelo núcleo.
Após reabrir, a vida caiu de **11.519.957.679 para 11.519.957.478**; as filas
avançaram estritamente mantendo os itens em processamento. As baterias novas
receberam **14.210→41.615 HE** do RTG e **35.000→170.000 HE** do radgen.
O relatório declara `excluded_phases: ["automation"]`; isto não fecha as duas
dependências de motor acima nem declara prontidão para merge.

Solar/Stirling (36 verificações com reabertura) e combustão passaram no mesmo
cliente/pacote. Demais regressões estão em andamento. Índice intermediário:
`.agent-tmp/rtg/manual-validation.json`; logs de suite, Clippy e build nessa pasta.
Cliente SHA-256 `ceabd253a8740bf2a21b55cef97c6585b2e6c035d082b4af32d8970d255ba59b`;
pacote SHA-256 `81f63a2d0cc4be0eba92ab819339c29244e079dc97dd26c74f116f39f80cf361`.

O roteiro `tools/test-game/native_radioisotopes.py`, integrado ao driver
`run-native-reactors.py radioisotopes`, usa montagem/tela, recusa de funil no RTG, distribuição por funil no radgen, baterias reais,
`/tick sprint` para decaimento natural e reabertura com vida/progresso estritos.
Evidências desta branch ficam em `.agent-tmp/rtg/`.

## Lote 5 de conteúdo — registro histórico

O lote acrescenta 70 identidades originais: `ntm:machine_rtg_grey` (variável Java `machine_rtg`), `ntm:machine_radgen`, dez pellets RTG vivos, seis pellets esgotados e 52 resíduos do radgen (oito classes curtas e cinco longas, cada uma em normal/tiny/vivo/esgotado). Os dois blocos usam beehive com facing horizontal e honey_level 0..5 e ficam Inert; visual-gen mantém `auto: false`. Texturas e nomes vêm do fonte; os nomes dos resíduos incluem a classe isotópica que o original exibia no tooltip.

O catálogo chega a 2.771 IDs (429 blocos e 2.342 itens). As dez receitas originais dos pellets vivos foram destravadas, totalizando 948 receitas vanilla. A montadora ganha as construções de RTG e radgen, chegando a 443 operações. A tabela Rust correspondente foi regenerada pelo machine-gen, com aprovação da frente de lógica.

`machine-recipes/radioisotopes.json` conserva os contratos algorítmicos:

- RTG: 15 slots, buffer de 100.000 HE, saída antes da geração do tick, calor limitado a 600 TU com decaimento ou 200 sem ele e conversão de 5 HE/TU. Os dez pellets preservam calor, produtos e tempo de vida, incluindo os dois valores de calor para ouro/chumbo conforme a configuração.
- O cálculo do tempo de vida reproduz a multiplicação float32 e as conversões para long do Java, com 48.000 ticks/dia e 100 dias/ano. A flag de decaimento é `enable528 || doRTGsDecay`; a escala de potência usa `enable528 || scaleRTGPower`. Defaults do fonte: decaimento ligado, escala de potência desligada. Configuração externa não foi aplicada ao snapshot.
- O calor de cada pellet é lido antes de decrementar/substituir o item. O campo `PELLET_DEPLETION` é por stack. O fonte inicializa um stack sem NBT com a vida completa; um compound existente sem esse campo lê zero. Isso está registrado para a lógica preservar.
- Radgen: 12 filas independentes, 12 saídas e buffer de 1.000.000 HE. A tabela contém 28 combustíveis reais, com potência por tick, duração e resíduo. Tiny tem 1/10 da potência e 1/10 da duração do tamanho normal, conforme o fonte. Sucata nuclear não deixa resíduo; a gema radioativa deixa diamante.
- O radgen continua processando com o buffer cheio e descarta a energia excedente. A capacidade da saída é verificada ao iniciar a fila; o fonte não a verifica novamente na conclusão. Slots, estado e filas persistidos, portas, redes e funcionamento nativo são trabalho separado.

Os pellets esgotados têm `setContainerItem(plate_iron)` no NTM. As cinco receitas literais de reciclagem devolveriam essa carcaça; o item de conteúdo atual não oferece esse retorno. Por isso permanecem como cinco gaps explícitos (`native crafting remainder required`), e o catálogo registra `crafting_remainder: ntm:plate_iron`. O caso de níquel já fica no ramo condicional do fonte. Não foi publicada uma reciclagem que perdesse a placa nem criada uma receita temporária.

O extrator de hazards passou a ler `registerRTGPellet`, o overload de Item para todas as metas dos resíduos novos e o ItemStack específico do pellet esgotado de neptúnio. As taxas são do NTM: por exemplo, pellet padrão 15 RAD/s, chumbo 15.000 RAD/s, neptúnio esgotado 3,75 RAD/s; resíduos curtos 30 RAD/s e longos 5 RAD/s, com os respectivos fatores de tamanho/decaimento. HOT/BLINDING do helper também são conservados como dados. Nenhuma taxa nem outro hazard de item já existente foi alterado (comparação por ID com a base C2).

A taxa RTG exportada é a base fresca. O `HazardModifierRTGRadiation` interpola até o alvo conforme a vida restante; os dez casos estão registrados como lacuna dinâmica para a camada nativa. O snapshot não apresenta a taxa base como dose correta para todo estado de decaimento.

Solução reaproveitada: catálogo de energia, parser de enums/registro, o escritor de snapshots existente, tradutor de receitas, extrator de hazards, machine-gen e o verificador em jogo. Nenhuma lógica de geração ou radiação Rust foi implementada neste lote.

Validação: 177 testes em cada seed 1/42 e os validadores de conteúdo e receitas passaram. A prova no ntm-next com bundle NTM/Recipes, em Xvfb e sem áudio, passou em 115 verificações: blocos nos níveis 0/5 e todos os 70 IDs recebidos e conferidos no inventário em lotes. Detector de rejeição/panic limpo. A captura confirma as texturas de fallback; não afirma funcionamento ou fidelidade dos multiblocos.

Evidências locais: `content-gen-evidence/energy-radioisotopes/`. Pacote persistente: `dist/ntm-content-energy5.hmod`, 6.641.553 bytes, SHA-256 `04b51c6e02a4be3336cf9630e96b2c5a6c00c6149c9000cce9abc0274e6edf29`.

O check do upstream e o do fork passaram no mesmo pacote. A comparação de hazards com a base C2 confirmou 70 entradas novas e zero alterações de taxas/efeitos antigos. O mundo descartável da prova foi removido; instalações e mundos reais não receberam o pacote.
