# O NTM como mod nativo (migração para o Horizon atual)

O Horizon atual não roda mais mods WebAssembly. Mod com código é um **bundle
horizon-mixins** compilado junto com o jogo (`docs/native-mods.md` do Horizon).
O plano completo está no artifact `horizon-migracao` do Traycer. Aqui fica o
que o repositório tem dele.

## Onde está

| Caminho | O que é |
| --- | --- |
| `native/ntm/mixins/ntm/horizon-mixin.toml` | O bundle: o crate da lógica e os hooks nomeados |
| `native/ntm/mixins/ntm/*.rs` | Um helper pequeno por hook |
| `native/ntm/compatibility.json` | Os commits fixados: Horizon (`ntm-next`), horizon-mixins 0.6, horizon-mods (Recipes) |
| `tools/package-native.sh [SAÍDA]` | O bundle pronto, com o `crates/ntm-core` copiado como `[[crate]]` |
| `tools/build-native.sh` | O jogo com o NTM e o Recipes oficial dentro |

O crate fica fora do bundle no repositório e é copiado no empacotamento. Os
campos herdados do workspace (`version.workspace`, `serde.workspace`...) viram
literais, porque um crate de bundle só aceita dependências de registro que já
estão no `Cargo.lock` do Horizon (`serde` e `serde_json` estão).

**Um crate só.** Um crate de bundle não pode depender de outro por `path`
(horizon-mixins recusa com `E_CRATE_DEPENDENCY`), e um crate de bundle não pode
ser o `dependent` de outro. Por isso o antigo `crates/ntm-explosion` virou o
módulo `ntm_core::explosion`, com os seus testes (`crates/ntm-core/tests/explosion_*.rs`)
e exemplos (`examples/explosion_*.rs`).

## Construir

```sh
BUILD_DIR=<pasta grande> agent-run env QUEUE=cargo bash tools/build-native.sh
```

1. Exporta o commit de `ntm-next` (`HORIZON_NEXT`, `NEXT_REV`) para `$BUILD_DIR/source`. Nunca usa o checkout em si.
2. Compila o horizon-mixins no commit fixado, uma vez.
3. Junta os bundles em `$BUILD_DIR/mods`: o do NTM, por `package-native.sh`, e o `recipes` do horizon-mods no commit fixado.
4. Roda `horizon-mixins apply` para `$BUILD_DIR/stage`.
5. Roda `cargo build --locked --release -p horizon-client`, o comando do próprio release, sem a feature `mods`.

O binário sai em `$BUILD_DIR/target/release/horizon-client`.

## Hooks em uso

| Hook | Versão | Helper | O que faz |
| --- | --- | --- | --- |
| `world_lifecycle` | 1 | `lifecycle.rs` | Ao abrir um mundo, loga `ntm native: N machine blocks` (a L0). |
| `block_entity` | 1 | `be.rs` | Um bloco de máquina posto recebe o block entity, com o estado novo em JSON sob `ntm` no compound. |
| `block_entity_ticks` | 1 | `ticks.rs` | Os block entities das máquinas fazem tick. |
| `block_entity_container` | 1 | `container.rs` | Os slots, as faces e o menu vanilla de cada máquina. |
| `block_entity_tick` | 1 | `tick.rs` | O tick do servidor: estado, slots e data do menu passam pelo `ntm_core::native`. |

A cola fica no módulo `ntm_glue` (`glue.rs`), que converte `ModBlockEntity` e `ItemStack` para os tipos simples do `ntm_core::native` e de volta. Um slot que a lógica não mudou mantém a pilha do jogo, com todos os componentes.

## A L1: a fornalha elétrica

`ntm_core::native` roda a fornalha elétrica (`machine_electric_furnace_off`) no menu vanilla de fornalha: entrada (slot 0, por cima), bateria (slot 1, pelos lados) e resultado (slot 2, por baixo). Os 4 data slots são energia, máximo, progresso e total, desenhados como a chama e a seta. A energia vem da bateria criativa no slot (`charge_from_slot`); as redes de energia entram na L3.

O template das máquinas é `beehive`, que não abre menu. Até o patch de menus da V2, o jogador não abre a tela; os funis fazem o papel dele na prova.

## Prova da L0 (30/10)

- Build: `ntm-next` `f867e476`, horizon-mixins `5140cdf8`, horizon-mods `ae217daa`. O `apply` encontrou 1 crate, 6 injeções e 2 módulos (NTM e Recipes); o `cargo build --release` levou 5 min 19 s.
- Jogo em Xvfb com o `ntm.hmod` do lote 4 de energia (já com `components`, da C1), num mundo novo: o log traz `ntm native: 89 machine blocks` ao abrir o mundo.
- O script repete o que o launcher faz e o horizon-mixins exige: tira os links do export (o `CLAUDE.md` do Horizon é um) e põe o commit do build no `horizon_revision` de cada bundle copiado.

## Prova da L1 (30/10)

`tools/test-game/native_l1.py <jogo> montar|conferir <horizon-ctl>`, no jogo de `build-native.sh` (`ntm-next` `f867e476`, 10 injeções e 3 módulos, 4 min 20 s):

- **montar:** por `/setblock`, a fornalha com um funil de 8 minérios de ferro por cima, um funil com a bateria criativa ao lado e um funil embaixo para um baú. O block entity nasceu com o estado `ntm`. Em 25 s, 5 lingotes no baú e a fornalha no meio do sexto (progresso 13 de 100, energia cheia).
- **conferir**, depois de fechar e reabrir o jogo: o baú com os 8 lingotes, a fornalha com a bateria no slot 1 e o mesmo estado. Um minério novo no funil virou o nono lingote.
