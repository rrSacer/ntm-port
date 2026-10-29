# Máquinas de processamento (Marco 3)

2026-10-29. Fluidos em [fluidos.md](fluidos.md). Caminhos de Java relativos a
`ntm-src/src/main/java/com/hbm/`.

## Ordem e cobertura

A ordem segue os itens que cada máquina destrava: `source_recipe_machine` em
`content/ids.json`, com o número de exclusivos entre parênteses.

| # | Máquina | Itens destravados | Receitas no port (puladas na extração) |
| ---: | --- | ---: | --- |
| 1 | Montadora `machine_assembly_machine` | 120 (106) | 398 (233 chamadas da fonte) |
| 2 | Centrífuga `machine_centrifuge` | 45 (45) | 54 (25) |
| 3 | Planta química `machine_chemical_plant` | 32 (25) | 47 (30) |
| 4 | Centrífuga de gás `machine_gascent` | (a cadeia UF6/PuF6) | transcrita à mão |

O triturador (*shredder*) tem 56 (47) e fica sugerido para depois.

As receitas vêm do agente de importação, que resolve itens e OreDict:
`content/machine-recipes/*.json`, schema 1, com `skipped` e motivo.
`tools/machine-gen/build.py` as converte para o formato embutido
(`crates/ntm-core/data/machines/*.json`), com os nomes em pt_BR. Cada
ingrediente aceita as alternativas do OreDict.

As receitas puladas dependem de itens fora do catálogo, laços de bedrock,
probabilidades, pressão ou configurações. Nenhuma receita foi inventada.

## Montadora, planta química, centrífuga (`crafter.rs`)

Uma lógica só sobre as tabelas de receita.

| | Montadora | Planta química | Centrífuga |
| --- | --- | --- | --- |
| NTM | `TileEntityMachineAssemblyMachine` | `TileEntityMachineChemicalPlant` | `TileEntityMachineCentrifuge` |
| Tanques | 1 + 1, 4.000 mB (ajustados à receita) | 3 + 3, 24.000 mB | — |
| Escolha da receita | lista (até 512 linhas) | lista | pelo item posto (`recipe_for`) |
| Energia | HE/t e ticks da receita; buffer `max(HE/t·100, 100k)` | idem | 200 HE/t, 200 ticks, 100k |

A tela tem três visões, porque a tela de mod do Horizon não rola: o que passa
de ~200 px some.

- **Máquina:**
  - energia;
  - a receita (ícone e nome);
  - progresso, ou "Falta: …" com o que falta;
  - "Pronto: N";
  - botões: escolher receita, pôr itens da mão, retirar o pronto, fluidos e
    recipientes (ou retirar as entradas).
- **Receita:** a lista. Um clique numa linha é `ui-events.select(id, linha)`,
  que vira a ação `LIST + linha`.
- **Fluidos:** os tanques, despejar e encher recipientes.

Regras:

- **Itens:** "Pôr itens da mão" tira até 16 de uma vez, uma transação por
  item, e só o que a receita escolhida aceita. O pronto volta por `Give`. Ao
  quebrar, entradas e saídas caem (transações `Drop`).
- **Saídas por chance** (`ChanceOutput`): o sorteio é um hash fixo de
  (receitas feitas, saída), não o aleatório do mundo.
- **Trocar de receita:** o progresso volta a 0; os tanques passam ao tipo da
  receita e, se o tipo muda, o conteúdo se perde (como no NTM).
- **Portas:** de energia e de fluido, abertas nas 6 faces.

**Fora, documentado:**
- upgrades (velocidade, energia, overdrive);
- os pools de `blueprints`: todas as receitas ficam selecionáveis;
- pressão;
- a fábrica de 4 módulos;
- o multibloco 3×3×3, que aqui é um bloco só.

## Centrífuga de gás (`gascent.rs`)

`TileEntityMachineGasCent.java` e `GasCentrifugeRecipes.java:24-34, 94-96`.

- **Tanques:** um tanque real de 2.000 mB (UF6, PuF6 ou lama do Watz), que vira
  um tanque de "pseudofluido" de 8.000 mB.
- **Estágios:** cada um tem consumo, produção, o próximo pseudofluido e suas
  saídas:

  | Estágio | Consome | Produz | Próximo | Saídas |
  | --- | ---: | ---: | --- | --- |
  | NUF6 | 400 | 300 | LEUF6 | 1 U238 |
  | LEUF6 | 300 | 200 | MEUF6 | U238 + fluorita |
  | MEUF6 | 200 | 100 | HEUF6 | U238 |
  | HEUF6 | 300 | 0 | — | 2 U238 + U235 + fluorita (**só com o upgrade**) |
  | PF6 | 300 | 0 | — | Pu238 + 2 Pu-mix + fluorita |
  | MUD | 1000 | 500 | MUD_HEAVY | chumbo em pó + poeira |
  | MUD_HEAVY | 500 | 0 | — | ferro em pó + poeira + resíduo nuclear |

- **Tempo e energia:** 150 ticks a 200 HE/t; com `upgrade_gc_speed`, 80
  ticks a 300 HE/t. O upgrade entra e sai por transação.
- **Cascata:** a cada 10 ticks, a centrífuga passa a saída para a que está
  atrás dela (oposta ao `facing`), se ela usa o mesmo fluido real. No fim de
  uma linha LEUF6, 600 mB viram 6 pepitas de combustível de urânio e 1
  fluorita. A cascata é do servidor, que vê as vizinhas
  (`fluid_server.rs::gascent_cascades`).
- **Desvio:** o tanque real aceita despejar célula ou tanque pela mão, além da
  rede (no NTM, só a rede). A tela não tem slot de identificador: o tipo vem do
  recipiente.

## Pontes `m1_temp__`

Nenhuma sai neste marco: nenhuma das 10 ficou produzível com as máquinas
novas sem quebrar a cadeia.

| Ponte | Máquina real no M3? | Por que fica |
| --- | --- | --- |
| `machine_epress` | sim: montadora `ass.epress` | pede 2 `part_generic__piston_hydraulic`, sem receita no port; e placa de aço só sai da montadora a partir de aço |
| `machine_battery`, `machine_electric_furnace` | não (sem receita de montadora extraída) | — |
| `stamp_iron_plate`, `stamp_iron_wire` | não (bigorna) | — |
| `aluminium_from_ore`, `red_copper_alloy` | não (beneficiamento, alto-forno) | — |
| `machine_wood_burner`, `red_cable`, `wiring_red_copper` | não | o agente de conteúdo já importou as receitas originais de bancada (`bulk__*`); retirar a ponte é decisão de quem cuida das receitas |

## Otimização feita no caminho (item 0)

`same_look` agora usa `blockstate::shows`, sem os dois `BlockState::parse`,
equivalente em todos os pares de 16 estados de teste. Por máquina carregada a
cada tick, medido no harness (incluindo o `get-block` de ~490):

| | Antes | Depois |
| --- | ---: | ---: |
| Com pool | 7.526 fuel | 2.086 fuel |
| Sem pool | 9.259 fuel | 2.187 fuel |

(`spikes/m2-fuel/results-m3.txt`)

## Verificação

- **`cargo test --workspace`** (153) e **`clippy -D warnings`**: limpos.
- **Mundo falso:**
  - montadora: a receita escolhida pela lista; 5 lingotes postos da mão por
    transação; sem energia nada acontece; com energia, 5 placas; retiradas para
    o inventário; um item fora da receita fica na mão;
  - planta química: 8 tanques de ar despejados; 500 mB de oxigênio; encher um
    tanque de gás com menos que 1.000 mB é recusado;
  - centrífuga: a receita escolhida pela entrada;
  - quebrar solta o que a máquina tinha;
  - centrífuga de gás: um estágio; a cascata de 4 até HEUF6 (U235 só com o
    upgrade); PuF6; duas em fila no mundo falso passam LEUF6 para a de trás.
- **Em jogo** (Xvfb `:9`, mudo; `tools/test-game/m3.py`; mundo criativo,
  seed 1768123):
  - **Montadora** com bateria (+10% de teste, modo saída) do lado: receita
    escolhida na lista (`docs/img/m3-assembly-list.png`), 5 lingotes de ferro
    pela mão, 5 placas no inventário em 21,6 s (5 × 60 ticks mais os cliques;
    `m3-assembly-working.png`).
  - **Rede de fluido + planta química:** ver [fluidos.md](fluidos.md).
  - **Centrífuga de gás:** 2 células de UF6 pela mão (as 2 vazias voltaram),
    uma pepita de U238 retirada após o primeiro estágio.
  - Nenhum "took too long" no log.
  - **Aceitação visual do pacote de conteúdo** (opacidade das faces):
    `docs/img/m3-visual-*.png`.
- **Não verificado em jogo:** a centrífuga comum. Ela usa a mesma lógica da
  montadora, que tem teste no mundo falso.

## Comportamento a saber

Pôr um bloco encostado numa máquina com o botão de usar também abre a tela
dela, porque o cliente abre a máquina mirada quando "usar" é apertado. É o
desenho do M1 (tecla R ou usar). Isso pode surpreender ao montar fileiras de
máquinas.
