# Bombas do NTM (Marco 2, modo otimizado)

2026-10-29. Rust/WASM, sem mudar o motor do Horizon. Fonte: `ntm-src` (NTM
1.7.10); caminhos de Java relativos a `src/main/java/com/hbm/`.

| Peça | Onde |
| --- | --- |
| Motor de explosão (cratera, fallout 2,5R, energia esparsa, persistência) | `crates/ntm-core/src/explosion` (ver também [explosoes.md](explosoes.md)) |
| Catálogo de bombas e a bomba como máquina (componentes, tela) | `crates/ntm-core/src/bomb.rs` |
| Explosão convencional (vanilla), efeitos, config de dois modos, zonas de radiação | `crates/ntm-core/src/blast/` |
| Gatilhos, detonação, dano, radiação, `/ntmbomba` (servidor) | `crates/ntm-core/src/server/bomb_server.rs` |
| Detonador (cliente) | `Client::detonator` + `mods/ntm-logic-client/src/lib.rs` |
| Testes no mundo falso | `crates/ntm-core/src/server/tests/bomb_tests.rs`, `crates/ntm-core/tests/explosion_fallout.rs` |
| Medição de fuel | `spikes/m2-fuel/` (`run.sh`, `results.txt`) |

Mudanças em arquivos compartilhados, todas pequenas:

- `server.rs`: o campo `blasts`, a chamada `bombs_tick` entre `simulate` e
  `run_txs`, o braço `C2S::Detonator`, um aviso de colocação de bomba no
  começo de `block_event`, e `mod bomb_server`.
- `radiation_server.rs`: o Geiger mostra a taxa do inventário mais a da zona
  (`area_rate`) em `radiation_done`; a dose da zona é do servidor
  (`area_dose`, a cada 20 ticks). Combinado com o agente da fundação.
- `mods/ntm-logic-server/src/lib.rs`: o pool allocator, `/ntmbomba`, e
  espectador conta como `creative` (imune, como na amostra do inventário).
- `proto.rs`: `C2S::Detonator { dim, target, link }`.
- `machine.rs`: a variante `Bomb`. `registry.rs`: as 11 bombas (`kind: Bomb`,
  sem porta, sem nível) e `BOMB_POWERED`, com teste contra `ids.json`.
- `persist.rs`: `bombs: BombWorld` no save do mundo (`#[serde(default)]`).
- `client.rs`: só o método `Client::detonator`.
- `server/tests.rs` (mundo falso): `playsound`/`particle` aceitos como
  `effect`/`damage`, e `mod bomb_tests`.

Os ids, texturas, receitas e blocos de bomba são do agente da importação em
massa (`content/`, `tools/content-gen`): blocos com template
`minecraft:redstone_lamp` e `light: 0`, `lit=false/true` com o mesmo modelo.

## Bombas escolhidas

| Bloco | NTM | Explosão | Raio/força | Componentes (`isReady`) |
| --- | --- | --- | --- | --- |
| `ntm:tnt_ntm` | `BlockTNT` | vanilla `createExplosion` | 10 | — (pavio de 80 ticks) |
| `ntm:dynamite` | `BlockDynamite` | vanilla | 8 | — |
| `ntm:semtex` | `BlockSemtex` | vanilla | 12 | — |
| `ntm:c4` | `BlockC4` | vanilla | 15 | — |
| `ntm:nuke_boy` | `NukeBoy` | MK5 (cratera) + fallout | 120 | blindagem, alvo, projétil, propelente, ignitor |
| `ntm:nuke_gadget` | `NukeGadget` | MK5 + fallout | 150 | fiação, 4 lentes de 1ª geração, núcleo grande |
| `ntm:nuke_man` | `NukeMan` | MK5 + fallout | 175 | unidade de disparo, 4 lentes de 1ª geração, núcleo |
| `ntm:nuke_mike` | `NukeMike` | MK5 + fallout | 250 | 4 lentes + núcleo (pronta); + núcleo revestido, deutério, resfriamento |
| `ntm:nuke_tsar` | `NukeTsar` | MK5 + fallout | 175 / 500 | 4 lentes + núcleo (175); + núcleo da Tsar (500) |
| `ntm:nuke_n2` | `NukeN2` | MK5 `statFacNoRad` (sem fallout, sem radiação) | 200 | 12 cargas N² |
| `ntm:nuke_fstbmb` | `NukeBalefire` | cratera com fogo, radioativa | 250 | ovo de fogo maldito, bateria de faísca |

Fontes: `config/BombConfig.java:8-21`, `tileentity/bomb/TileEntityNuke*.isReady`,
`blocks/bomb/Nuke*.igniteTestBomb`, `blocks/bomb/Block{TNT,Dynamite,Semtex,C4}`.

Interpretações:

- **Ivy Mike:** `NukeMike.igniteTestBomb` ignora o raio que recebe e sempre usa
  `mikeRadius` (NukeMike.java:133). O port segue o código: os dois estágios
  dão 250. A Tsar usa o raio recebido: 175 com o primeiro estágio, 500 com o
  segundo.
- **Balefire:** o NTM tem uma tela com temporizador; aqui ela dispara por
  redstone ou detonador, como as outras. A cratera usa o motor (raio 250), sem
  escória, com fogo em metade das bordas expostas, e deixa zona radioativa.
  `EntityBalefire` tem outro algoritmo (destrói tudo) que não foi portado.
- **Convencionais:** a explosão é a do vanilla (16³ raios, resistência), sem
  drop de blocos, com dano vanilla aos jogadores.
- **Fora:** F.L.E.I.J.A., Prototype, Solinium (MK3, antimatéria), mísseis e
  cargas (`det_charge`, `charge_*`).

## A bomba como máquina

Uma bomba é uma máquina (`Kind::Bomb`): os componentes entram e saem pelas
**mesmas transações** das máquinas (`TakeHeld` tira 1 da mão, `Give` devolve),
com save, reenvio, rollback por crash e drop ao quebrar. Nada de `give`/`clear`
fora delas. Ao detonar, a bomba some do mundo e do save junto com os
componentes, como `clearSlots` no NTM.

Tela (tecla **R**, ou usar o bloco com a mão vazia): uma linha por tipo de
slot (12 cargas N² são uma linha, `7/12`), o estado (`Pronta (raio 120)` ou
`Faltam componentes`), **Pôr o componente da mão** e **Retirar um componente**
(o último posto). A primeira versão, com um botão por slot, não cabia na tela
(`docs/img/m2-boy-gui-v1.png`); a atual está em `docs/img/m2-boy-gui.png`.
Cargas mostram a força e **Acender o pavio (4 s)**.

## O que dispara

- **Redstone.** O bloco é uma lâmpada de redstone (template): o vanilla põe
  `lit=true` quando ela recebe sinal, e o servidor lê isso a cada tick por
  `get-block` (só as bombas carregadas; ~970 fuel por bomba). Dispara na
  **borda de subida**, como `onNeighborBlockChange`: uma bomba já ligada quando
  o mundo carrega não explode; uma colocada já com sinal explode. Uma nuke só
  explode se estiver pronta; uma carga acende o pavio (80 ticks).
- **Detonador** (`ntm:detonator`, `ItemDetonator`): o client mod reivindica o
  item (`input.claim-items`), então usar não coloca nem balança nada.
  Agachado e mirando uma bomba (até 5 blocos): liga. Em pé: dispara a bomba
  ligada, a qualquer distância, se o chunk dela estiver carregado e ela na
  mesma dimensão. Numa carga, acende o pavio. O vínculo é por jogador, no save
  do mundo (`BombWorld.links`); o NTM guarda no NBT do item, que o mod não lê.
  Respostas no chat: ligado, sem bomba, não está pronta, chunk não carregado,
  outra dimensão.
- **Pavio** das cargas: vive nos dados da máquina (salvo), anda no `tick`.
- `/ntmbomba teste <raio> [n2|balefire|forca]`: detona na posição do jogador
  (só no criativo), para testes. O raio vai de 1 a 500; a força (`forca`), de
  1 a 20. Fora disso o comando é recusado antes de qualquer coisa.

Nada dispara enquanto a bomba tem uma transação de item em andamento, ou uma
já feita que ainda não foi conferida com o mundo depois de um reinício (um
componente que a queda desfez). O `lit` também espera, então a borda de subida
não se perde. O detonador responde "Aguarde: os componentes da bomba ainda
estão sendo confirmados" e confere se o bloco ainda é a bomba.

## Fila de detonação

Uma bomba disparada sai do mundo (máquina e bloco) e entra numa fila no save
do mundo (`BombWorld.queue`), **no mesmo save** em que sai. Uma queda logo
depois do disparo não perde a explosão: a fila volta com o save.

A cada tick, a fila dispara em ordem enquanto cabe no fuel do tick (`fuel`,
22 M):

- uma carga custa ~13 M·(p/15)²: a explosão vanilla de força 15 mede 12 M no
  harness, então duas C4 nunca vão no mesmo tick;
- uma nuke só entrega o plano ao motor, que monta um por tick.

A nuke fica na fila até o motor tê-la gravada na pasta. Se o servidor cair
antes disso, ela é entregue de novo, sem repetir efeitos.

## Explosões

Nukes usam o motor do `ntm_core::explosion`, uma por mundo: o id é `w<mundo>n<n>`,
retomado só no mundo certo (`resume_matching`), e as chaves `nx.*` ficam na
mesma pasta do ntmlogic. Ao sair do mundo, as explosões ficam no storage e
continuam quando ele voltar.

Perfil de efeitos com os blocos do content mod: bordas expostas da cratera
viram `ntm:sellafield_slaked` (o fallout do NTM escoria até 1,25R, a cratera
inteira), areia 5% `ntm:waste_trinitite`, troncos `ntm:waste_log`, tábuas
`ntm:waste_planks`, fogo em 1/5 até 0,65R. N2: nenhum efeito. Balefire: fogo.

### Fallout de 2,5R

`crates/ntm-core/src/explosion/fallout.rs` (desenho em explosoes.md §8), segunda
fase da mesma explosão, depois das bordas:

- Colunas em anéis de R a 2,5R; cada coluna começa a busca do topo em
  `max(topo das 3×3 vizinhas já feitas) + 8` e sobe 16 por vez enquanto não for
  ar; desce até 3 sólidos.
- Regras do `FalloutConfigJSON` por distância `d` (fração de 2,5R): madeira →
  `waste_log` (eixo mantido), tábuas → `waste_planks`, folhas/plantas → ar até
  65%; folhas além disso → `waste_leaves[persistent=true]`; pedra/terra/areia →
  `sellafield_slaked` até 50% (grama até 45%); areia além disso, 5% trinitite;
  argila → terracota; camada `ntm:fallout[layers=1]` com chance
  `0,1 − (d − 0,7)²`; fogo 1/5 até 65%.
- **Chunk descarregado:** a coluna fica pendente por chunk. A explosão termina
  e os pendentes viram um `Linger` (só a lista de chunks, sem o plano), feito
  quando um jogador chega a 160 blocos, conferido a cada 40 ticks quando nenhuma
  explosão está ativa. Persistido (`phase=linger`).
- **Seções da cratera em chunk descarregado** não seguram mais o fallout nem a
  fila: o fallout começa, as seções adiadas são tentadas a cada 20 ticks, e
  uma explosão que só espera chunks (`idle`) cede a vez às outras.
- Retomada: com diário (R ≤ 150) a cratera é reaplicada (cobre o mundo que
  voltou ao autosave) e o fallout continua do anel salvo; o que o rollback
  apagou do fallout antes desse anel não volta (aceito).
- Fora da API: troca de bioma e blocos caindo.

### Energia esparsa

O plano não guarda mais nada por voxel do disco: energia em buffers `u16` por
coluna só na frente da espiral (devolvidos 3 blocos atrás dela, ou retidos até
8 MB perto de chunks descarregados, pais do recálculo), remoções em faixas por
coluna, só as bordas expostas, e a espiral gerada anel a anel. Resultado
**idêntico** ao denso (mesmos removidos, leituras e visitas em R=25, 50, 100,
150), e `Plan::new` passou de O(R²) (21 M de fuel em R=150) para O(R).

| R | Pico de memória do plano | Denso seria | Buffers vivos | Ticks de 34 M estimados |
| ---: | ---: | ---: | ---: | ---: |
| 150 | 3,3 MB | 29,7 MB | 2.787 | 187 |
| 175 | 4,6 MB | 45,8 MB | 3.255 | 294 |
| 250 | 8,8 MB | 119,6 MB | 4.713 | 837 |
| 500 | 25,5 MB | 600,3 MB | 9.410 | 4.604 |

(`cargo run --release -p ntm-core --example explosion_big -- 150 175 250 500`,
terreno sintético, nativo.) O teto de um mod é 64 MB.

**Persistência leve para R > 150:** o diário de R=500 passaria de 10 MB (a pasta
tem 4 MB para tudo). Acima de 150 grava-se só um bit por seção escrita
(`nx.<id>.w`, ~10 KB em R=500); a retomada refaz o plano e pula as seções já
escritas. O que elas removeram virou ar, então a cratera além delas sai um
pouco maior: medido +6,3% de blocos num R=30 interrompido com 10+ seções
escritas (teste `lite_persistence_skips_written_sections`).

## Efeitos (modo otimizado)

Tudo por comandos vanilla que o client mod roda (`S2C::Run`, `execute in
<dim>`), meia dúzia de linhas por explosão, qualquer que seja o raio:

- `playsound` de explosão (volume pelo raio: ouvido a 16·volume blocos) e de
  trovão; o pavio aceso toca `entity.tnt.primed`;
- `particle minecraft:flash{color:[…]}` (verde na balefire), `explosion_emitter`,
  e um "cogumelo" de `campfire_signal_smoke` (tronco) e `large_smoke` (copa);
- náusea de 8 s para jogadores dentro de R (não criativo).

`efeitos off` desliga tudo; `particulas N` (0–200) regula a quantidade.

**Somente o modo otimizado está ativo (01/11/2026).** O visual aprimorado foi
arquivado por decisão do usuário. O placeholder `Enhanced`, a seleção de backend
e o aviso de fallback foram removidos. Configurações salvas com `Aprimorado`
são lidas como `Otimizado`, preservando raio, orçamento, efeitos e demais opções.
Comandos novos aceitam apenas `modo otimizado`.

## Dano e radiação

- **Dano da nuke:** `ExplosionNukeGeneric.dealDamage(…, 2R)` do NTM, a cada
  tick do MK5: 250·(2R − d)/2R se a linha até o jogador não está obstruída.
  Aqui: a cada 10 ticks, por 100 ticks, `damage <nome> <n> minecraft:explosion`;
  obstrução por amostragem (24 pontos). Criativo não leva.
- **Carga:** dano vanilla, `(i² + i)/2 · 7 · 2p + 1` com `i = (1 − d/2p) ×
  exposição`. A exposição é a de `Explosion.getSeenPercent`: a fração de 45
  raios (grade 3 × 5 × 3 sobre a caixa de 0,6 × 1,8 do jogador) que chegam ao
  centro sem bater num bloco com colisão. É calculada antes de os blocos
  saírem, na ordem do vanilla. Atrás de uma parede de obsidiana, nenhum dano.
- **Pulso de radiação** (`EntityNukeExplosionMK5.radiate`, 10 primeiros ticks):
  Σ 2,5 M/(5t+1) ≈ 3,78 M RAD sobre (blindagem × distância²), até 2R; a
  blindagem é a soma da resistência ao longo da linha (48 amostras). Entra
  direto na dose (`radiation::add_dose`, teto 2.500 RAD) e salva na hora. N2 e
  convencionais: nenhum.
- **Zona radioativa** (contaminação de área): cada nuke com fallout (e a
  balefire) deixa uma zona de raio 2,5R no save do mundo, com
  `pico = zona × R/100` mRAD/s no marco zero (padrão 10 RAD/s para R=100),
  caindo com o quadrado da distância e pela metade a cada `meiavida` ticks do
  mundo (padrão 12.000 = 10 min). O servidor soma a taxa da zona onde cada
  jogador está à dose dele a cada segundo (criativo, espectador e a carência
  de entrada ficam de fora), sem depender da amostra do cliente; o Geiger mostra
  a taxa somada à do inventário, e as doenças saem com a próxima amostra.
  Zonas abaixo de 1 mRAD/s somem.

## Configuração

`/ntmbomba config [global|mundo] [<chave> <valor> | limpar]`. A global fica na
pasta (`bombcfg`), a do mundo no save dele e vale sobre a global.

| Chave | Padrão | O quê |
| --- | --- | --- |
| `modo` | `otimizado` | somente `otimizado` |
| `fallout` | on | fase de 2,5R |
| `efeitos`, `particulas` | on, 24 | sons e partículas |
| `dano` | on | dano de explosão |
| `radiacao`, `zona`, `meiavida` | on, 10000, 12000 | pulso e zona |
| `orcamento` | 8192 | escritas de bloco por tick |
| `raiomax` | 500 | teto do raio das nukes |
| `fuel` | 22 | fuel estimado por tick, em milhões (4–34) |

`/ntmbomba status`: configuração, explosões ativas (linha de progresso),
fallouts pendentes, zonas e as últimas detonações. `/ntmbomba cancelar`
para as explosões do mundo.

## Fuel: o que o teste em jogo achou, e as medições

No primeiro teste em jogo, o server mod foi **desligado pelo Horizon** ("took
too long") no tick seguinte à detonação do Little Boy, e depois na retomada.
A causa: o motor planeja 34 M de fuel *estimado* por tick (o teto do Horizon é
50 M), as estimativas estavam abaixo do real e o resto do ntmlogic também
gasta. Medido no harness `spikes/m2-fuel` (wasmtime 48.0.7, o do Horizon, com
`consume_fuel`; as strings de bloco são alocadas no guest pelo host, como no
`cabi_realloc`; terreno sintético):

| Medição | Antes | Depois |
| --- | ---: | ---: |
| Plano: real ÷ estimado | 1,26× | 1,00× (custos recalibrados) |
| Pior tick da cratera, orçamento 34 M | 53,5 M | 40,3 M |
| Pior tick do fallout, orçamento 34 M | 192,1 M | 45,2 M |
| Retomada de R=120 num tick | > 50 M (mod desligado em jogo) | `resume()` 3,0 M; restauração em passos |
| Fallout com chunks descarregados além de 110 blocos (R=120) | 125,4 M (mod desligado em jogo) | 13,0 M |

Correções: custos por leitura/visita/escrita/conferência e do diário
recalibrados; o fallout classifica por cache FNV por material (o `classify`
compara padrões de texto) e gera cada anel por faixas de `dz` (antes varria um
quadrado inteiro por anel: O(k²)); `section_cells` monta por baldes de altura
em vez de ordenar; as colunas de chunk pendente custam o que custam (o
conjunto de pendentes usa FNV; antes cada uma valia 100 na estimativa, e num
anel quase todo descarregado o laço pulava centenas de milhares por tick); a
retomada restaura o diário em passos com orçamento; e o
ntmlogic usa **22 M** de orçamento (`fuel`), o que deixa o pior tick medido em
~30 M e ~20 M para o resto do mod.

Medição final (orçamento 22 M, fallout ligado, diário ligado):

| | Sem pool | Com pool |
| --- | ---: | ---: |
| `get-block` + comparação | 591 | 490 |
| `BlockState::parse` | 3.910 | 3.094 |
| reconciliação de máquina (get-block + parse + propriedades) | 5.732 | 4.815 |
| leitura do `lit` de uma bomba (sem parse) | 1.069 | 968 |
| `set-blocks` como o mod (clone + `BlockChange`), por bloco | 882 | 648 |
| cratera R=50, por leitura | 1.163 | 1.045 |
| carga vanilla força 15 (uma explosão) | 20,9 M | 19,4 M |
| motor R=100: pior tick / média | 33,3 M / 21,8 M | **29,4 M / 19,1 M** |
| motor R=150: pior tick / média | 35,5 M / 21,2 M | **29,6 M / 18,5 M** |
| retomada R=120: `resume()` / pior tick depois | — | 3,0 M / 30,2 M |
| motor R=120, terreno dos testes (árvores, água, cavernas): pior / média | — | 32,2 M / 18,8 M |
| idem, sem chunk além de 110 blocos: pior (fallout) | — | 30,0 M (13,0 M) |

### Pool allocator: antes e depois

Instalado no `ntmlogic-server` (`#[global_allocator]`, só em wasm32;
`ntm_core::Pool` reexporta `ntm_core::explosion::pool::Pool`). Ganhos medidos na
tabela acima: 16–27% por operação, ~12% no motor e −12% a −17% no pior tick.
O ganho é menor que o das rampas do S7 (≥ 4,3×) porque, no ntmlogic, o custo
dominante passou a ser o `BlockState::parse` (~3 k fuel por estado), não o
alocador. **Recomendação para a fundação:** a reconciliação de máquinas
(`same_look`) faz dois `parse` por máquina a cada 20 ticks; comparar a string
diretamente, como o scan das bombas faz com `lit=true]`, cortaria ~80% desse
custo.

A carga vanilla gasta ~20 M num tick só; no tick em que uma carga explode, as
nukes não andam (`charged_at`).

Medições do harness são de um processo sozinho, sob a fatia
`ntm-builds.slice` (CPU só com o que sobra): os números de fuel não dependem
da carga, os tempos sim.

## Verificação

`cargo test --workspace` (134 testes: 115 do `ntm-core`, 19 do
`ntm_core::explosion`) e `cargo clippy --workspace --all-targets -- -D warnings`
limpos. Testes novos:

| Teste | O quê |
| --- | --- |
| `bomb::tests` (3) | raios do NTM por estágio, componentes em slots e de volta, pavio |
| `blast::vanilla::tests` (2) | cova rasa em pedra, obsidiana intacta, chunk descarregado para o raio; dano por distância |
| `bomb_tests::a_little_boy_assembled_from_the_hand_goes_off_by_redstone` | montagem pela tela com transações, redstone ignorada sem estar pronta, borda de subida, cratera com `sellafield_slaked`, dano, dose, zona e `area_rate`, storage limpo |
| `…a_charge_burns_its_fuse_then_digs_like_vanilla` | pavio de 80 ticks, som, explosão |
| `…the_detonator_links_and_sets_off_a_ready_bomb` | sem vínculo, liga, "não está pronta", detona; N2 sem escória nem zona |
| `…breaking_a_bomb_gives_its_components_back_once` | quebrar solta os componentes uma vez |
| `…an_explosion_resumes_after_a_restart` | reinício com a explosão no meio: retoma e termina |
| `…settings_are_global_or_per_world` | config global/mundo, `limpar`, valor inválido; modo aprimorado recusado |
| `ntm_core::explosion` `fallout.rs` (5) | regras e limites por distância, retomada no meio do fallout com rollback, chunk pendente até um jogador chegar (e após reinício), persistência leve, cratera esperando chunk sem segurar fallout nem a fila |
| `fallout::tests::rings_cover_the_annulus_once` | anéis cobrem o anel uma vez |
| `engine.rs` (10, antigos) | inalterados, passam com o plano esparso |
| `registry::tests` | bombas de `ids.json` = `BLOCKS`, template `redstone_lamp`, `lit`, componentes e detonador existem |

## Revisão do M2 (segunda, independente)

Cada achado tem um teste que falhava no código entregue no M2 e passa
agora. Os do motor estão em `crates/ntm-core/tests/explosion_review.rs`; os do
servidor, em `crates/ntm-core/src/server/tests/review_m2_tests.rs`.

| Achado | Correção | Teste |
| --- | --- | --- |
| P1-1: R=500 com chunks descarregados, pico de 3.420,8 M de fuel no planejamento | filas por coluna de chunk, conjuntos em vez de buscas lineares, administração limitada e descontada do fuel (`explosoes.md` §11) | harness `spikes/m2-fuel` (abaixo) |
| P1-2: várias cargas ou planos no mesmo hook | fila de detonação admitida pelo fuel do tick; o motor monta um plano por tick | `p1_2_charges_ending_together_go_off_within_the_tick_budget`, `p1_2_one_plan_per_tick` |
| P1-3: cada `Linger` com o fuel inteiro | os fallouts pendentes dividem fuel e escritas do tick, em rodízio | `p1_3_lingering_fallouts_share_the_tick` |
| P1-4: um chunk pendente do fallout de uma vez (66 mil leituras) | cursor por coluna, dentro do fuel e das escritas de cada chamada | `p1_4_a_pending_chunk_is_done_in_pieces_within_budget` |
| P1-5: detonar antes de conferir as transações dos componentes | nada dispara com transação em andamento ou ainda não conferida | `p1_5_no_detonation_before_the_components_are_confirmed` |
| P1-6: explosão na fila some no reinício | fila no save do mundo, junto com a saída da bomba; estado inicial gravado na criação | `p1_6_a_detonation_survives_a_crash_in_its_tick`, `p1_6_queued_explosions_survive_a_restart` |
| P1-7: diário e cabeçalho sem proteção contra gravação rasgada | estado em dois slots com geração e checksum, blocos selados, linger gravado antes de apagar o diário | `p1_7_a_torn_write_never_loses_the_explosion` (rasga cada gravação), `p1_7_a_torn_linger_header_keeps_the_pending_fallout` |
| P1-13: `teste 100 forca` alocava ~165 MiB | faixa conferida antes de tudo; força vanilla limitada a 20 | `p1_13_test_blasts_are_bounded`, `vanilla::power_is_held_to_the_maximum` |
| P2-8: retomada leve no fallout descartava seções adiadas | `crater_left` no estado: a retomada refaz o plano e termina a cratera | `p2_8_lite_resume_in_fallout_keeps_deferred_crater` |
| P2-9: carência eterna para outros jogadores | relógio de entrada por jogador | `p2_9_area_dose_grace_is_per_player` |
| P2-10: detonador sem conferir o bloco | confere o bloco antes de disparar | `p2_10_the_detonator_checks_the_bomb_is_still_there` |
| P2-11: `nx.*` fora da conta da quota | limites por tick e no total, dados pelo servidor; persistência leve quando não cabe | `p2_11_explosions_share_the_folder_quota`, `p2_11_the_journal_keeps_to_its_share` |
| P2-12: dano de carga sem obstrução | exposição do vanilla (45 raios) | `p2_12_charge_damage_is_blocked_by_walls`, `vanilla::exposure_sees_through_air_not_walls` |

O pior tick de R=500 com o disco quase todo descarregado foi de **3.420,8 M para
26,3 M** de fuel, e nenhum caso medido passa de 29,4 M (tabela em
[explosoes.md](explosoes.md#11-revisão-do-m2-trabalho-por-tick-limitado)).

**Em jogo** (Xvfb `:9`, mudo; `tools/test-game/m2review.py`; mundo criativo,
seed 1768123; nenhum "took too long" no log):

- **Cargas:** três C4 ligadas pela mesma fileira de blocos de redstone
  explodiram em três ticks diferentes, cada uma com a explosão de força 15
  inteira (593, 1.037 e 931 blocos).
- **Nuke:** `teste 40` terminou em 36 ticks, com fallout de 26.392 colunas.
- **Reinício:** `teste 60`, saída do jogo (`quit`) já no fallout. O estado ficou
  em `nx.<id>.s0`/`s1` e no anel `e0..e2`. Ao reabrir, `explosion resumed`: a
  cratera foi reaplicada (14 ticks), o fallout continuou e a explosão terminou
  em 64 ticks. Ficaram 32 chunks do fallout esperando um jogador.

### Em jogo

Mudo (`HORIZON_NO_AUDIO=1` — o log diz "no sound: audio off"; `options.txt`
com master/music 0,0; `mute.py`), Xvfb `:9`, `agent-run --user sacer` com
`nice 10`, pasta em `/tmp/ntm-test-game/m2-bombs` (`tools/test-game/run-xvfb.sh`),
mundo novo de sobrevivência criativa, seed 1768123, perto de (3530, 70, −1400).
Os `.hmod` em `/tmp/ntm-m2-dist`.

- **C-4 por redstone:** colocada pelo jogador, bloco de redstone ao lado →
  `lit=true` → pavio (1 linha de som) → 4 s → explosão vanilla força 15: 1.462
  blocos, 8.192 leituras, 0 comandos falhos; a caixa medida perdeu 1.453 blocos.
- **Little Boy pela tela:** os 5 componentes pela tecla R e pelo botão, cada um
  saindo do inventário por transação; a tela mostrou os 5 ok e "Pronta (raio
  120)" (`docs/img/m2-boy-gui.png`).
- **Detonador:** agachado + uso na bomba ligou (`target Some([3532,70,-1400])`);
  em pé, a 70 blocos, detonou.
- **Cratera R=120:** pronta em **218 ticks**, ~11 s de relógio (≈ 20 TPS) com
  o PC carregado (load 3,5), sem o mod cair (com o orçamento de 22 M e as
  correções acima); 242.845 blocos removidos, 22.118 efeitos de borda, zona de
  raio 300 com 10,3 RAD/s no centro. 128 seções em 8 colunas de chunk a ~190
  blocos do jogador estavam descarregadas e ficaram esperando.
- **Retomada em jogo:** o jogo foi fechado com a explosão esperando chunks;
  ao reabrir, `explosion resumed`, a cratera foi reaplicada em 48 ticks (ar
  sobre ar) e o fallout começou — depois de corrigidos os dois estouros de
  fuel que o teste achou (retomada num tick só; colunas puladas no fallout).
- **Fallout em jogo:** 6.550 colunas feitas nos chunks carregados, 850 chunks
  pendentes. Ao ir até o norte da cratera, as 128 seções que faltavam foram
  escritas (3.248/3.248, `explosion done`), e o fallout que restava virou
  `Linger` ("fallout esperando 823 chunk(s)": 27 feitos pela aproximação).
  `ntm:sellafield_slaked` no fundo da cratera; todos os estados de bloco do
  perfil (`waste_log[axis=y]`, `waste_leaves[persistent=true]`,
  `fallout[layers=1]`…) aceitos pelo jogo.
- **Dano em sobrevivência:** a 6 blocos de uma C-4 (força 15), vida 20 → 0.
- **Zona radioativa em sobrevivência:** a ~70 blocos do marco zero, dose de
  840,48 → 892,16 RAD em ~30 s, mesmo com a amostra do Geiger sem confirmar
  (`docs/img/m2-zona-antes.png`, `m2-zona-depois.png`); acima de 800 RAD a
  doença da fundação matou o jogador ("withered away"). A zona decaiu de
  10,3 para 5,0 RAD/s no centro em ~20 min de jogo (meia-vida 10 min).
- **Achados do teste em jogo, todos corrigidos e cobertos:** os três estouros
  de fuel (seção "Fuel"); a tela com um botão por slot que não cabia; a
  cratera esperando chunk que segurava o fallout e a fila; a zona somando dose
  a um jogador criativo (`area_rate` ignora criativo e espectador, com a
  carência de entrada da amostra).
- O Geiger (HUD) só mostra a taxa da zona quando a amostra do inventário da
  fundação confirma; com o PC carregado, ela às vezes não confirmou ("sem
  confirmação", "tags do mod desconhecidas"). A dose da zona não depende disso.

## Storage

As chaves das explosões (`nx.*`) dividem a pasta do mod (4 MiB, 64 chaves,
1 MiB gravado por chamada de hook) com os saves dos mundos. O servidor dá às
explosões o que sobra:

- **bytes:** a quota menos o que os saves ocupam (`Worlds::usage`) e menos um
  save a mais, de margem;
- **chaves:** o limite menos as dos saves e 4 de margem;
- **por tick:** 1 MiB menos três saves do maior mundo e uma caixa de correio.

O aviso de "pasta quase cheia" conta as explosões também. Um diário completo
que não cabe passa à persistência leve (só as seções escritas, 1 bit cada). A
leve que falha tenta de novo a cada 10 s; a explosão continua sem ela.

## Limites

- Uma explosão por vez recebe o tick; R=500 leva ~4.600 ticks estimados (a
  ~22 M/tick, uns 7–8 min) — o NTM leva ~6,5 min só para coletar a Tsar.
- Chunks descarregados não são carregados à força: a cratera e o fallout neles
  esperam um jogador chegar perto (o NTM gera de forma síncrona).
- Troca de bioma, blocos caindo, empurrão (knockback) e fogo no jogador ficam
  de fora (sem API).
- O vínculo do detonador é por jogador, não por item.
- Bombas postas por comando (`setblock`) não são vistas até alguém abri-las
  (como as máquinas do M1).
