# Onda 2 — sumidouros e processamento simples sobre as redes existentes

Branch `onda2-k3-c`. Blocos que só dependem das redes de energia, de fluido (com pressão) e de itens que o port já tem, escolhidos depois do cruzamento dos blocos ainda `Kind::Inert`
contra o que as outras frentes entregaram (`onda2-k3-s2`: radiólise, cíclotron, câmara de exposição, anilhador, orbus, funil, FEL, SILEX, rockmill, forja de chaves, gerador industrial).
Ordem: ralo, micro-ondas, chaminés, depois a avaliação das três fábricas (`machine_precass`, `machine_chemical_factory`, `machine_assembly_factory`).

| # | Bloco | Estado |
| --- | --- | --- |
| 1 | `machine_drain` | Concluída (lacunas abaixo) |
| 2 | `machine_microwave` | Pendente |
| 3 | `chimney_brick`, `chimney_industrial` | Pendente |

Scripts e evidências: `/mnt/ssd120/sacer/so/c` (`run_drain_proof.py`, `mutate_drain.py`, `drain-gameplay.json`, `drain-proof-report.json`); o corte de gravação roda sempre com
`HORIZON_NO_EAGER_SAVE=1`.

---

## 1. `ntm:machine_drain`

### Contrato fonte

- `MachineDrain.java` (`BlockDummyable`): `{0,0,2,0,0,0}`, `getOffset() == 0`, um volume de 1x1x3 que se estende para trás do núcleo (a frente do núcleo, no sentido do bloco, fica livre); só o
  núcleo tem `TileEntity`. `onBlockActivated`: sem agachar, com um identificador de fluido na mão, `tank.setTankType(type)` (o identificador `none` destipa e esvazia); agachado devolve `true` e não faz nada.
- `TileEntityMachineDrain.java`: um tanque de 2 000 mB (`Fluids.NONE`), receptor de fluido (`getReceivingTanks`, **prioridade `LOW`**: a rede serve primeiro os receptores normais e o ralo só pega a sobra; um receptor só aceita o próprio tipo, então um ralo sem tipo não
  aceita nada); portas (`getPorts`): o núcleo aberto na frente (`dir`) e nos dois lados (`dir` girado para cima e para baixo), nunca em cima nem embaixo (`canConnect`).
  `updateEntity`: com o tanque cheio de algo, `toSpill = max(fill / 2, 1)` sai do tanque a cada tick e `FluidTrait.onRelease(SPILL)` roda para cada traço do fluido.
- Traços com `onFluidRelease`: `FT_VentRadiation` (`incrementRad(world, x, y, z, mB * radPerMB)`) e `FT_Polluting`. `FT_Amat` (antimatéria) faz o Java explodir o bloco (10) e parar.

### Implementação

- [`crates/ntm-core/src/drain.rs`](../crates/ntm-core/src/drain.rs) (`Kind::Drain`, `Data::Drain`): tanque `Tank` de 2 000 mB sem tipo; `FluidNode` que só demanda o próprio tipo e só até a folga;
  `step` joga fora a metade (mínimo 1 mB) e guarda a radiação do tick (`FT_VentRadiation`, `vent_radiation` do fluido × mB), que o servidor leva ao chunk (`transaction.rs::drain_tick`, mesmo caminho
  do anilhador). Portas: `registry::fluid_ports` (`Drain::faces`: a frente e as duas laterais, a partir do `facing`).
- O identificador de fluido tipa o ralo por `native_use_item_as` (aceita o núcleo de um ralo, além dos dutos); no cliente o `glue.rs` converte a célula clicada no núcleo (`machine_cell_core`) e diz se o
  jogador usou o item agachado (o estado do próprio jogador, também de um convidado LAN). **O ralo agachado não tipa e a ação é tomada** (`true`, como o Java); **o duto continua tipado do mesmo jeito agachado ou não** (ver lacunas). O identificador `none` destipa e esvazia o ralo (e não tipa duto). Um tipo novo esvazia o tanque (`FluidTank.setTankType`).
- `fluid_priority`: `PRIORITY_LOW`, a mesma do Java.
- Conteúdo: `visual.json` → arquivo de máquina com o multibloco (`0,0,0` a `0,0,2`) e o modelo OBJ `drain.obj` com a textura original (`RenderDrain`, caso 2: 90°).

### Provas

- **Testes (`ntm-core`):** `drain::tests` (7) e `server::tests::drain_tests` (7): metade por tick até o último mB (`1000, 500, 250, 125, 63, 32, 16, 8, 4, 2, 1, 0`); só aceita o tipo dado e a folga; um tipo novo
  esvazia; radiação por mB no chunk (`uf6`) e nenhuma da água; antimatéria fica no tanque; as portas dos quatro sentidos (frente e lados, nunca atrás nem em cima); salvar e reabrir mantém o tipo.
  Em servidor: um barril de água na lateral esvazia, um no fundo ou em cima não, um ralo sem tipo não pega nada; **um receptor de prioridade normal é servido antes do ralo, que só pega a sobra, com um duto na frente do ralo
  ligado ao fornecedor** (a revisão pegou a prioridade esquecida); o clique agachado: o ralo o toma sem tipar, o duto é tipado, `none` destipa o ralo e não tipa duto.
- **Mutantes (`mutate_drain.py`):** **20/20 mortos** (divisão por 3, sem o mínimo de 1, sem a regra da antimatéria, radiação plana, qualquer tipo, sem tipo, tipagem inerte, 2 faces, lado trocado, tanque de
  1 000, `fluid_take` que drena, registro inerte, sem o ramo do ralo no identificador, radiação não enviada, portas vazias, radiação acumulada, prioridade normal, tipar agachado, `none` não tratado, duto que aceita `none`). `D02` e `D06` sobreviveram/travaram e ganharam teste
  (laço limitado e a demanda do "sem tipo").
- **Cliente nativo:** `horizon-client` release SHA-256 `aaa9796bdf2b3c1e63b80d8af6cb798dc9774c5b91f0ea636a241b03bb925fc4`.
- **Prova em jogo (`run_drain_proof.py`):** ralo de 3 células posto pelo motor, dois barris de água (um na lateral, uma porta; um em cima, que não é porta). **9/9 checks `ok: true`**: o ralo nasce sem tipo e o volume tem as três
  células; um ralo sem tipo não pega nada; **o clique com o identificador agachado não tipa**, o simples (numa célula do fundo do volume) tipa; o barril lateral esvazia completamente e o ralo fica com ~0 mB; o
  barril de cima não perde nada; **o clique agachado num duto o tipa** (não regride) e o do ralo não.
- **Corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`), 7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0`, `.mca`
  inalterados antes da escrita. Base salva com o ralo sem tipo; no ato o identificador tipa com um clique real. Antes da escrita as duas reaberturas voltam ao último save (sem tipo); depois da escrita (e em `none`) as duas
  reaberturas mantêm o tipo `water`. O bloco do ralo está sempre lá.

### Lacunas nominais

- **Dutos, `FluidDuctBase.handleActivation`:** o Java tipa **só o duto** num clique simples e, agachado ou com `TOOL_CTRL`, **a rede inteira** (`changeTypeRecursively`, até 64 passos, só os dutos do tipo anterior); com `TOOL_ALT` copia o tipo do duto para o identificador. Aqui o duto é tipado só nele mesmo, agachado ou não, e nem a recursão nem `TOOL_ALT`/`TOOL_CTRL` existem (já era assim antes do ralo).
- **Antimatéria:** o Java explode o bloco (`newExplosion` 10, fogo e fumaça) e para; aqui o fluido fica no tanque e nada acontece (o motor tem as explosões de máquina só por efeitos gravados; ficou para
  quando houver um efeito genérico de explosão).
- **Poluição (`FT_Polluting`) e a poça de óleo (`oil_spill`, só com `toSpill >= 100` e 1 chance em 20):** o port não tem `PollutionHandler` nem o bloco `oil_spill` no catálogo.
- **Partículas do cliente** (a coluna de gás e o respingo) e a mensagem "Changed type to ..." no chat.
