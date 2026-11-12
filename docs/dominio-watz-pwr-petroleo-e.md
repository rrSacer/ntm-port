# T8: domínio puro de Watz, PWR e petróleo E

As duas linhas `pub mod` do `lib.rs` estão numa só linha, para o merge conflitar menos. Módulos novos, sem ligação ao tick, ao save nem aos menus: `crates/ntm-core/src/watz_pwr/{function,heatable,watz,pwr}.rs` e `petroleo_e.rs` (mais `data/machines/flare.json`, cópia da tabela de conteúdo). Os blocos continuam `Inert` no registro.

## O que entrou
- **Watz**: 12 pellets (constantes, curvas, lama = valor/2), tick da coluna empilhada (refrigerante, reação, lama, queda e troca de pellets, resfriamento de 1%), portão da bomba e da redstone, conversão da carcaça (4 parafusos Dura), drops ao quebrar, verificação da estrutura do núcleo, geometria das portas.
- **PWR**: 15 combustíveis (com a peculiaridade real: o construtor ignora o rendimento, todos ficam em 1e9; o 250 000 000 declarado de `BFB_*` é registrado e não usado), varredura da estrutura (`assemble`/`floodFill` na ordem exata, limite 4096, quirks de orientação 0/1), `setup` (conexões, limite de 80 dissipadores, capacidade), tick do controlador (carga, barras, fluxo, calor, refrigerante, moderadores, derretimento), tabela de fluidos aquecíveis.
- **Petróleo E**: `Hephaestus` (varredura térmica por camada; o limite inferior é o parâmetro `min_y`, 0 como no Java nos testes de paridade — **a ligação ao jogo deve passar o `min_y` real do mundo, que tem y negativo**, triplo com `ore_volcano`, conversões) e `Flare` (vent/burn, upgrades, teto de 100 000 HE, `fuel_traits` dos 156 fluidos).

## Prova
`tools/watz-pwr-oracle/` compila trechos literais das classes Java originais e grava entradas e saídas (doubles em bits). `crates/ntm-core/tests/watz_pwr_parity.rs` (12 testes) compara tudo tick a tick: 19 colunas Watz, a estrutura do núcleo Watz em cada uma das 110 posições (330 casos de dano, mais 9 horários), 28 estruturas PWR (inclui linhas de combustível com alcance 14 a 18), 46 cenários de fissão (15 combustíveis, 9 coolants, subida e descida das barras, dissipadores 13 a 17, chunks não carregados, depleção, 3 derretimentos, ping de satélite), 45 execuções do Hephaestus e 365 do flare (inclui som e dano de fogo). Bit a bit, exceto o acumulador `progress` do PWR (log10/pow), comparado em até 4 ulp. Mutações em constantes e regras (0,99, 0,999, penalidade, janela de 20 ticks, lama/2, limite de carga, parada no casing, refletor, subida das barras, ceil dos dissipadores, alcance de 15, ping, som, fogo, cada grupo de blocos do núcleo Watz) fazem os testes falharem.

## Lacunas nominais
- Falha do Watz: só o evento `MudOverflow`; estilhaços, colunas aleatórias, radiação no chunk, conquista dependem de entidades/efeitos de mundo.
- Derretimento do PWR: só barras e centro; corium, explosão e `SatelliteRayScan` ficam para quem ligar.
- Slot identificador de fluido, impressora PWR, áudio, OpenComputers/ROR (exceto regras numéricas), GUI, `TileEntityProxyCombo`, descoberta de portas, bateria do flare, poluição, fogo/dano, partículas, som.
- Regras de inventário/automação do PWR (`isItemValidForSlot`, `canExtractItem`, lados acessíveis), proxies de fluido e inventário de `BlockPWR` (`transferFluid`, `getDemand`, só meta 1), tanques de entrada/saída e `canConnect` do Watz, PWR e Hephaestus.
- Portas e formas do flare (`TilePortShapes.flare`) não extraídas. Volcanic lava e `ore_volcano` ainda não existem no catálogo.

## Falta ligar (depois da Onda 0)
`machine.rs`/`native_server.rs`: criar os `Kind` dos 8 IDs, chamar `watz::tick_stack`, `PwrController::tick`, `Hephaestus::tick`, `Flare::tick`; persistir o estado; menus e fluidos; aplicar os eventos acima.

O oráculo não executa o som e o fogo reais: o stub só registra que foram chamados, e o teste compara se e quando.
