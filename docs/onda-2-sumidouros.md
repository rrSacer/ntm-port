# Onda 2 — sumidouros e processamento simples sobre as redes existentes

Branch `onda2-k3-c`. Blocos que só dependem das redes de energia, de fluido (com pressão) e de itens que o port já tem, escolhidos depois do cruzamento dos blocos ainda `Kind::Inert`
contra o que as outras frentes entregaram (`onda2-k3-s2`: radiólise, cíclotron, câmara de exposição, anilhador, orbus, funil, FEL, SILEX, rockmill, forja de chaves, gerador industrial).
Ordem: ralo, micro-ondas, chaminés, depois a avaliação das três fábricas (`machine_precass`, `machine_chemical_factory`, `machine_assembly_factory`).

| # | Bloco | Estado |
| --- | --- | --- |
| 1 | `machine_drain` | Concluída (lacunas abaixo) |
| 2 | `machine_microwave` | Concluída (lacunas abaixo) |
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
- **Cliente nativo:** `horizon-client` release SHA-256 `64bb736823e347334835243c2604594fcf84420d4bf90663d6116aef6c467082` (a ser regravado no build limpo, depois do commit do micro-ondas: ver o fim desta seção).
- **Prova em jogo (`run_drain_proof.py`):** ralo de 3 células posto pelo motor, dois barris de água (um na lateral, uma porta; um em cima, que não é porta). **10/10 checks `ok: true`**: o ralo nasce sem tipo e o volume tem as três
  células; um ralo sem tipo não pega nada; **o clique com o identificador agachado não tipa**, o simples (numa célula do fundo do volume) tipa; o barril lateral esvazia completamente e o ralo fica com ~0 mB; o
  barril de cima não perde nada; **o clique agachado num duto o tipa** (não regride) e o do ralo não.
- **Corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`), 7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0`, `.mca`
  inalterados antes da escrita. Base salva com o ralo sem tipo; no ato o identificador tipa com um clique real. Antes da escrita as duas reaberturas voltam ao último save (sem tipo); depois da escrita (e em `none`) as duas
  reaberturas mantêm o tipo `water`. O bloco do ralo está sempre lá.

### Lacunas nominais

- **Dutos, `FluidDuctBase.handleActivation`:** o Java tipa **só o duto** num clique simples e, agachado ou com `TOOL_CTRL`, **a rede inteira** (`changeTypeRecursively`, até 64 passos, só os dutos do tipo anterior); com `TOOL_ALT` copia o tipo do duto para o identificador. Aqui o duto é tipado só nele mesmo, agachado ou não, e nem a recursão nem `TOOL_ALT`/`TOOL_CTRL` existem (já era assim antes do ralo).
  O identificador `none` num duto também continua recusado (o Java destipa o duto com `setType(NONE)` quando o tipo difere); só o ralo o aceita.
- **Antimatéria:** o Java explode o bloco (`newExplosion` 10, fogo e fumaça) e para; aqui o fluido fica no tanque e nada acontece (o motor tem as explosões de máquina só por efeitos gravados; ficou para
  quando houver um efeito genérico de explosão).
- **Poluição (`FT_Polluting`) e a poça de óleo (`oil_spill`, só com `toSpill >= 100` e 1 chance em 20):** o port não tem `PollutionHandler` nem o bloco `oil_spill` no catálogo.
- **Partículas do cliente** (a coluna de gás e o respingo) e a mensagem "Changed type to ..." no chat.

---

## 2. `ntm:machine_microwave`

### Contrato fonte

- `MachineMicrowave.java` (`BlockMachineBase`, um bloco só, `rotatable`): o `TileEntityMicrowave` em toda a célula; `TilePortShapes.around`: uma porta, as seis faces.
- `TileEntityMicrowave.java`: 3 slots (0 entrada, 1 saída, 2 bateria), 50 000 HE de buffer, `consumption` 50, `maxTime` 300, velocidade 0 a 5 (`handleButtonPacket`: botão 0 sobe, 1 desce, preso a 0..5).
  `updateEntity`: recebe energia, `chargeTEFromItems(slots, 2, ...)`; com `canProcess` (velocidade > 0, ao menos 50 HE, uma entrada com resultado de forno em que **a entrada ou o resultado é comida** (`ItemFood`), e a saída vazia ou do mesmo item
  e com espaço): **a 5 de velocidade o bloco é destruído e explode** (`ExplosionVNT` de 5); com `time >= 300` faz um resultado e zera; se ainda puder, gasta 50 HE e `time += speed * 2`. Só `power` e `speed` são salvos.
- `isItemValidForSlot` é `i == 0 && FurnaceRecipes.smelting().getSmeltingResult(item) != null` para a mão e o funil (o slot 2 não aceita nada); `canExtractItem` só o slot 1; lados: embaixo o slot 1, o resto o slot 0.
- GUI 176x168: entrada (80,35), saída (140,35, só retirar), bateria (8,53); botões em (43,25) e (43,43); barras de energia (34 px), de progresso (22 px) e de velocidade (34 px). `RenderMicrowave`: o prato gira a `speed * 100` graus por segundo quando `time > 0`.

### Implementação

- [`crates/ntm-core/src/microwave.rs`](../crates/ntm-core/src/microwave.rs) (`Kind::Microwave`, `Data::Microwave`): as regras do Java sobre as receitas do forno do port (`furnace_recipes.json`); a comida é a lista de `FOODS` (as carnes e os peixes crus e cozidos, as batatas, o kelp e a carne de
  glífido); a bateria do slot 2 usa as regras provadas de `energy_network::charge_te_from_items` (a carga fica no item). Consumidor de energia (`demand = max - power`), porta de todas as faces.
- Telas: os valores das barras usam o truque `2p - 1` de `2 * px` (cada pixel exato); os botões 0 e 1; o dado de giro (velocidade com `time > 0`) toca a animação do prato (uma por velocidade, a `3,6 / v` s por volta, em torno de `(0,575, 0, -0,45)`).
- Slots: a regra de aceitar da entrada é a lista de entradas do forno, o slot 1 só retira e o slot 2 não aceita nada (`slot_filters`); o funil: embaixo o slot 1, nos outros lados o slot 0.
- Conteúdo: `visual.json` → arquivo de máquina, tela, modelo `microwave.obj` (o corpo, a janela e o prato), animação do prato.

### Provas

- **Testes (`ntm-core`):** `microwave::tests` (11) e `server::tests::microwave_tests` (4): quando termina cada velocidade (151, 76, 51 e 39 ticks), 50 HE por tick de trabalho, só comida, parar sem velocidade ou com 49 HE, a saída de outro item ou com dano ou
  cheia, a pilha de resultados, a 5 **não roda**, os botões presos, a bateria criativa e a de chumbo no slot 2, as faces e os slots, os pixels exatos das três barras, salvar só `power` e `speed`; em servidor: cozinha na energia da rede com a velocidade
  dos botões e **a conta fecha** (a rede paga exatamente `3 x 75 x 50`), o que não é comida fica, a 5 fica parado e a 4 cozinha, a bateria posta por comando carrega, salvar e reabrir.
- **Mutantes (`mutate_microwave.py`):** **24/26 mortos**; os dois sobreviventes são equivalentes, não testáveis: `W23` (o valor da barra `2p` em vez de `2p - 1`) muda só a guarda contra o erro de ponto flutuante do `ceil`, que nenhum dos 35 e 23 pixels do teste dispara (o teste desenha cada pixel e confere), e `W24` (tirar o `.min(22)` do progresso) dá o valor `2*23-1 = 45` de `44`, que `screens::fill` já limita a 1,0: a mesma barra de 22 px. `W02` (50 HE → 40) sobreviveu porque o teste usava a constante e agora usa 50.
- **Prova em jogo (`run_microwave_proof.py`):** microwave posto pelo motor com cabo e uma bateria finita; **12/12 checks `ok: true`**: nasce sem velocidade e enche o buffer; um lingote de aço é recusado; o slot de bateria não aceita nada da mão; a carne crua entra; dois cliques no botão 0 põem a
  velocidade em 2; **3 carnes viram 3 carnes cozidas** e os itens se conservam; **a rede paga exatamente o trabalho** (lido com o jogo congelado: `3 x 75 x 50` HE); a velocidade 5 não roda; a 4 cozinha.

### Lacunas nominais

- **A velocidade 5:** o Java destrói o bloco e explode (`ExplosionVNT` de 5, com dano a entidades); o port não tem uma explosão que uma máquina possa pedir (só as dos efeitos gravados), então **a 5 a máquina não roda** (não cozinha e não gasta energia). É o que sobra de uma
  máquina que explode: a decisão de pôr a explosão pelos efeitos gravados fica para o motor.
- **A lista de comidas** é a das receitas de forno do port (os itens do `ItemFood` do Java que o forno usa); comidas do Java sem receita de forno aqui não entram.
- O som e a mensagem do cópia/cola de configurações (`ICopiable`) e o OpenComputers.
