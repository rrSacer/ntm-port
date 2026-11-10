# Estado da integração das frentes T2–T8 (ressalvas da revisão)

Registro dos achados da revisão da integração (`main` @ `27380b26`) e do que foi feito com cada um.

## `rustfmt`: o repositório não está formatado

Medido em 10/11/2026 sobre `main` @ `27380b26`, com `cargo fmt --all --check` (edição 2024 do workspace):
**122 dos 142 arquivos `.rs` têm diferenças, 1.976 trechos no total.** O repositório nunca foi mantido limpo
para o `rustfmt`; qualquer frase "rustfmt limpo" vale, no máximo, para um arquivo ou uma edição do `rustfmt`.

Afirmações dos relatórios das frentes que **não valem**:

| Relatório | Afirmação | O que o `cargo fmt --check` mostra |
| --- | --- | --- |
| T7 (cosméticos, `b626fde2`, no registro das frentes) | "clippy e rustfmt limpos" | `energy_network.rs` 3 trechos, `tests/energy_network_tests.rs` 1. Limpos só nas edições 2015 e 2021 do `rustfmt`, não na 2024 do workspace |
| T8 (revisão, P3-4, `lib.rs`) | "`pub mod watz_pwr; pub mod petroleo_e;` (rustfmt sem diferenças)" | O `lib.rs` tem 6 trechos com diferença na `main`. Duas declarações na mesma linha é justamente o que o `rustfmt` separa; a integração já as separou em duas linhas (`57a5015e`) |
| T8 (frente) | arquivos do Watz/PWR e do petróleo E formatados | `watz_pwr/watz.rs` 18, `watz_pwr/pwr.rs` 20, `watz_pwr/heatable.rs` 2, `watz_pwr/function.rs` 1, `petroleo_e.rs` 11, `tests/watz_pwr_parity.rs` 30 |
| T1 (`gen_rbmk_tables.py`, comentário "the repo is rustfmt-clean") | o repositório estaria limpo | `rbmk/persist.rs` 25, `rbmk/reactor.rs` 2, `rbmk/dials.rs` 1, `rbmk/rods.rs` 1, `tests/rbmk_oracle_parity.rs` 1 e, fora da frente, `server/rbmk_server.rs` 32 e `server/tests/rbmk_tests.rs` 54 |

Os arquivos da T6 (`doors_security.rs`, `tests/doors_security_tests.rs`) estão limpos na edição 2024.
Nenhum arquivo foi reformatado nesta rodada (reformatar tudo apagaria o histórico de autoria de 1.976 trechos);
a decisão de adotar o `rustfmt` no repositório inteiro, num commit só, é do dono do projeto.

## `bombs`: corrida do lever (corrigida no roteiro)

`tools/test-game/native_bombs.py`, `lever()`: a resposta do `interact` é lida **antes** de o servidor aplicar o clique
(mostra o estado antigo, `powered=false`, com `"changed": false`), então `physical_lever_powered` falhava por corrida e
não por defeito do jogo. Corrigido: o roteiro espera 4 ticks depois do teleporte e lê o bloco de novo, por até 20 ticks,
sem clicar duas vezes (um segundo clique desligaria a alavanca). O jogo não mudou.

## `items`: esperas em ticks de jogo

Todas as checagens falhavam por relógio de parede (`time.sleep` de 0,3 a 4 s contra os ticks do servidor), e a máquina
carregada (outras frentes, carga 20 a 50) mudava, a cada rodada, qual delas estourava. Medido, o que parecia "jogo lento"
tinha três causas, nenhuma no jogo:

1. **A gravação da evidência.** `Game.ctl` regravava o JSON de evidência inteiro a cada chamada; com o disco disputado,
   uma gravação levava de 2 a 21 s (soma de 504 s em 546 chamadas), e esse é tempo de parede em que o jogo anda sozinho.
   `Game.save_every` (o `native_items.py` usa 20 s) grava a cada tanto; cada checagem grava sempre, e o que a pausa segurou é
   gravado na saída (`atexit`), seja qual for o fim da execução. Sem `save_every` nada muda para os outros roteiros.
2. **O gesto de segurar o botão.** `mouse right down`, uma espera e `mouse right up` pelo lado de fora esticam a duração do
   gesto pelo atraso das chamadas. `use-item --hold` (o jogo aperta e solta) é atômico: 0,3 e 0,5 s nunca consumiram o
   doce, 1,8 s consumiu exatamente um, sete vezes em sete, com ~100 ticks de atraso nas chamadas em volta (sonda). O `use(g, s)`
   usa essa ação; os cancelamentos (`short_hold`) leem o `state` numa segunda conexão enquanto ela roda.
3. **O relógio.** `wait N` conta ticks do **servidor** (13 para 116 em `wait 100`); o `ticks` que o `state` e o `wait`
   devolvem é do cliente e corre à parte (22 para 208 no mesmo trecho). A idade de efeitos, damage do cantil, contador do RadAway
   e atraso de coleta é medida com `time query gametime` (`now(g)`), o relógio em que eles de fato andam.

O que mudou no roteiro, preservando o que cada checagem prova:

- `Game.ticks(n)` e `Game.command_ticks` (`native_game.py`; o `native_items.py` usa 6 ticks, os 0,3 s de antes); os
  `time.sleep` de jogo do `native_items.py` viraram `pause(g, segundos)` (20 ticks por segundo). Só o `settle` (consulta da tela
  sincronizada) fica em tempo de parede.
- `use()` espera o uso ser aplicado olhando o que está na mão (`used_up`): a resposta do `use-item` vem **antes** de o efeito
  aparecer (sonda: resposta `{}` e o inventário muda alguns ticks depois). Com teto de 3 s de jogo; um item que não se gasta
  (criativo, recusado) espera o teto.
- `cancel_does_not_consume`, `can_cancel_keeps_it`, `cancelled_bdcl_stops_its_gulps`: o botão fica 0,5 s (10 ticks de um uso de 32;
  0,55 s no BDCL) e o que o cancelamento fez continua sendo julgado pelo jogo, em toda tentativa; só se repete quando o `state` não
  chegou a ver o uso. Cada tentativa recoloca o item e limpa efeitos e dose.
- `thrown_container_waits_its_pickup_delay`: o atraso de 40 ticks corre do **arremesso**; a leitura "cedo" tem de sair em até 36 ticks
  do clique que arremessa, a "tarde" 42 ticks depois; se a cedo foi lenta, arremessa de novo.
- Efeitos curtos lidos tarde: o doce é comido de novo quando a leitura passa de 90 ticks (o veneno dura 300 e o `wither`, o mais curto,
  100); o cantil, o RadAway-flush e o `has_effects` aceitam a idade medida, e continuam recusando uma duração menor que a do Java
  por mais que isso.
- A taxa de dose (`rate_sample`) é a dose ganha em 30 s de jogo sobre os ticks medidos, por segundo, não mais o tamanho de um
  incremento isolado (que só se separa quando o roteiro responde em menos de um segundo de jogo).
- A maçã de euphemium e o doce passam a começar com a comida abaixo de 20 (fome de 3 s), em vez de depender de quanto tempo
  o roteiro anterior demorou; o RadAway seguinte limpa o tratamento (`ntmitem cure`).
- O mundo fica em dia fixo e sem mobs (`gamerule advance_time false`, `spawn_mobs false`, `time set noon`): com as esperas de
  centenas de ticks o roteiro passa de 19 mil ticks, anoitece e um zumbi mata o jogador de sobrevivência.
- `run-native-reactors.py` aceita `NTM_GAME_SIZE=LxA` (padrão 1280x720, inalterado) para uma janela menor.

Rodada com o código final (`NTM_GAME_WIDE=1`, máquina com carga 18 a 40 de outras frentes): **`items` completo, 187 checagens do
`setup`, 10 do `capture-state` e 7 do `check-state` (204), todas passam**, e nenhuma das repetições acima foi usada
(`holds`/`attempts` = 1 em todas).

## `door_sounds` das quatro portas

`catalog_k3.py` declara `block_properties.door_sounds` (`ntm:block.opendoor` ao abrir e ao fechar, `BlockModDoor.java:237/261`);
o gerador já aplicava `block_properties` ao bloco. `generate.py` e `visual-gen/generate.py` completos deixam os quatro
`door_*.json` iguais. `test_catalog_k3.test_simple_doors_keep_the_original_door_sound` confere a declaração e o arquivo do bloco;
removendo `door_sounds` de qualquer um dos dois o teste quebra.
