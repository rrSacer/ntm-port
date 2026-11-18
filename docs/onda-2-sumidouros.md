# Onda 2 — sumidouros e processamento simples sobre as redes existentes

Branch `onda2-k3-c`. Blocos que só dependem das redes de energia, de fluido (com pressão) e de itens que o port já tem, escolhidos depois do cruzamento dos blocos ainda `Kind::Inert`
contra o que as outras frentes entregaram (`onda2-k3-s2`: radiólise, cíclotron, câmara de exposição, anilhador, orbus, funil, FEL, SILEX, rockmill, forja de chaves, gerador industrial).
Ordem: ralo, micro-ondas, chaminés, depois a avaliação das três fábricas (`machine_precass`, `machine_chemical_factory`, `machine_assembly_factory`).

| # | Bloco | Estado |
| --- | --- | --- |
| 1 | `machine_drain` | Concluída (lacunas abaixo) |
| 2 | `machine_microwave` | Concluída (lacunas abaixo) |
| 3 | `chimney_brick`, `chimney_industrial` | Concluída (lacunas abaixo) |

Scripts e evidências: `/mnt/ssd120/sacer/so/c` (`run_drain_proof.py`, `run_microwave_proof.py`, `run_chimney_proof.py`, `mutate_*.py`, `*-gameplay.json`, `*-proof-report.json`); o corte de gravação roda sempre com
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
- **Cliente nativo (build limpa, árvore sem alterações, commit `f6ab567b`):** `horizon-client` release SHA-256 `01f48a822bf09c1062d5087cd454a5ee0e1a09ab48bf5938d68210f426a1a25b`, pacote de conteúdo `dist/ntm.hmod` SHA-256 `e5022cd022a278d57fbc26ac34153da949b977ccf7894056cbb69ce4fd3639d2`. As provas abaixo foram refeitas nesta build (as de builds anteriores, com a árvore ainda suja, ficam descartadas).
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
- **Corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`), 7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0`, `.mca` inalterados antes da escrita. Base salva com 3 carnes
  na entrada e a velocidade 0; no ato a velocidade vai a 2 e as 3 carnes cozinham. Antes da escrita as duas reaberturas voltam ao último save (velocidade 0, 3 na entrada, nada na saída); depois da escrita (e em `none`) as duas mantêm velocidade 2, 0 na entrada e 3 na saída, sempre com os 50 000 HE.
- **Cliente nativo:** o mesmo da seção 1 (build limpa, commit `f6ab567b`, SHA-256 `01f48a82…a25b`): gameplay 12/12 e corte 7/7 foram refeitos nele.

### Lacunas nominais

- **A velocidade 5:** o Java destrói o bloco e explode (`ExplosionVNT` de 5, com dano a entidades); o port não tem uma explosão que uma máquina possa pedir (só as dos efeitos gravados), então **a 5 a máquina não roda** (não cozinha e não gasta energia). É o que sobra de uma
  máquina que explode: a decisão de pôr a explosão pelos efeitos gravados fica para o motor.
- **A lista de comidas** é a das receitas de forno do port (os itens do `ItemFood` do Java que o forno usa); comidas do Java sem receita de forno aqui não entram.
- O som e a mensagem do cópia/cola de configurações (`ICopiable`) e o OpenComputers.

---

## 3. `ntm:chimney_brick` e `ntm:chimney_industrial`

### Contrato fonte

- `MachineChimneyBrick.java` e `MachineChimneyIndustrial.java` (`BlockDummyable`, `getOffset() == 1`): uma base de 3x3 com um eixo alto por cima (13 e 23 blocos); só o núcleo tem `TileEntity`
  (`TileEntityChimneyBrick`/`TileEntityChimneyIndustrial`, sobre `TileEntityChimneyBase`). `getAllPorts` (o formato do `flare`): as quatro células ao lado do núcleo, cada uma aberta só para fora.
- `TileEntityChimneyBase.java`: receptor de fluido sem tanque: `canConnect` só aceita `smoke`, `smoke_leaded` e `smoke_poison`; `getDemand` é 1 000 000 qualquer que seja a pressão; `transferFluid` devolve 0, ou seja, **consome tudo**
  que chega. Cada chegada liga a chaminé por `onTicks = 20` (a coluna de fumaça do cliente) e soma o fluido em `ashTick` (a de tijolo captura cinza; a industrial, `capturesSoot`, soma também a fuligem em `sootTick`). `updateEntity`:
  `ashLevelFly += ashTick` e `ashLevelSoot += sootTick` **no `TileEntityAshpit` logo embaixo do núcleo** (`int`: soma e corta como `int`); sem fosso embaixo a fumaça simplesmente some. Nada é salvo.

### Implementação

- [`crates/ntm-core/src/chimney.rs`](../crates/ntm-core/src/chimney.rs) (`Kind::Chimney`, `Data::Chimney`): `Body` (tijolo ou industrial) vem do bloco; `FluidNode` que demanda 1 000 000 mB dos três tipos de fumaça em qualquer pressão (`fluid_demand_at`/`fluid_give_at`) e
  nada dos outros; `fluid_give` liga por 20 ticks e acumula cinza (e fuligem, só a industrial); `take_ash` entrega e zera. As quatro portas: `chimney::PORTS` (células ao lado do núcleo com a face para fora), ligadas pelo `container_bridge`
  (`FLUID_PROXIES`) e pelas faces de `fluid_server` (o núcleo não conecta, cada porta só abre para fora).
- `oil_server::ash_tick`: a cinza e a fuligem tomadas entram no fosso de cinza embaixo do núcleo com a soma de `int` do Java (`(i64 + ash) as i32`).
- Conteúdo: `visual.json` → arquivo de máquina (multibloco `-1,0,-1` a `1,12|22,1`, deslocamento 1) e os modelos OBJ com as texturas originais.

### Provas

- **Testes (`ntm-core`):** `chimney::tests` (5) e `server::tests::chimney_tests` (6): só fumaça (`smoke`, `smoke_leaded`, `smoke_poison`) em qualquer pressão e tudo o que vem; água, vapor e ar não; 20 ticks ligada e recomeça com nova fumaça;
  as quatro portas para os quatro sentidos; em servidor, **um barril de fumaça na porta é esvaziado e a cinza (1 500) chega ao fosso embaixo** (a de tijolo sem fuligem, a industrial com as duas); as três fumaças na industrial; água ou fumaça
  numa célula que não é porta (em cima, ao lado de um canto, embaixo de uma porta) nada entra; as quatro portas, e sem fosso a fumaça some; a cinza dá a volta de `int`; só o corpo é salvo.
- **Mutantes (`mutate_chimney.py`):** **25/25 mortos** (qualquer fluido, `smoke_poison` fora, demanda 1 000, 19 ticks, sensível à pressão, `give_at` ignorado, tijolo com fuligem, industrial sem fuligem, sem cinza, aceita qualquer fluido,
  não liga, não conta ticks, `take_ash` vazio, porta faltando, face para dentro, corpos trocados, os dois registros inertes, sem as faces das portas, soma sem volta de `int`, sem fuligem no fosso, sem cinza tomada, camada da porta, sem porta na industrial,
  cinza limitada). `C02` (erro de compilação do vetor) e `C19` (a porta aberta em todas as faces) sobreviveram à primeira passada e ganharam correção do mutante e um caso de teste (a célula embaixo de uma porta).
- **Cliente nativo (build limpa, commit `f6ab567b`):** o da seção 1, SHA-256 `01f48a822bf09c1062d5087cd454a5ee0e1a09ab48bf5938d68210f426a1a25b`, pacote `e5022cd0…39d2`.
- **Prova em jogo (`run_chimney_proof.py`):** o port **não tem nenhuma fonte de fumaça** (nenhum barril ou máquina a produz), então o que o mundo mostra é a estrutura: **6/6 checks `ok: true`**; a chaminé de tijolo e a industrial postas pelo motor
  são uma chaminé com o corpo certo, a de tijolo tem 13 blocos e a industrial 23 (o bloco de cima está lá e o seguinte é ar), um barril de água na porta continua com 16 000 mB depois de 80 ticks (a porta só leva fumaça). O fosso de cinza embaixo não pôde ser posto
  com o alcance do jogador (um clique no fosso abre a tela dele; sem linha de visão por baixo da chaminé); a cadeia fumaça → cinza é a dos testes de servidor e dos mutantes.
- **Corte de gravação estrito (`HORIZON_NO_EAGER_SAVE=1`), 7/7 pontos `ok: true`** (`none`, `kill`, `kv_committed`, `save_begin`, `chunks_written`, `entities_written`, `world_saved`), `kv_items=0`. Base salva com a chaminé industrial e um baú de controle;
  nas duas reaberturas de cada ponto a chaminé está lá, com o corpo industrial, a célula mais alta do eixo e o baú.

### Achado do corte de gravação (não é das chaminés)

Colocar uma máquina **na última janela antes do corte** (a colocação como ato do corte) dá, em `chunks_written` e `entities_written`, um bloco que ainda não foi gravado (o baú de controle também é ar) **com o registro da máquina já gravado**: depois de duas
reaberturas e um save normal o registro continua num lugar de ar. Foi visto com a chaminé industrial e **o mesmo com o micro-ondas** (já aprovado) na mesma build; `none`, `kill`, `kv_committed`, `save_begin` e `world_saved` não mostram isso (a máquina não existe ou existe inteira). Parece ser a ordem entre
a camada do mod e os chunks (não investigada a fundo), comum a qualquer máquina, e não foi mexida aqui; as provas das chaminés, do ralo e do micro-ondas preparam a máquina na base e cortam em cima de um estado já gravado. Fica para o dono do motor/glue (não verifiquei o que um registro sem bloco ainda faz no servidor).

### Lacunas nominais

- **Poluição:** a porta da chaminé não tem poluição própria (`PollutionHandler.incrementPollution` da chaminé de tijolo e da industrial, com a fuligem); o port não tem `PollutionHandler`.
- **A coluna de fumaça** (`onTicks` no cliente, partículas) e o som; o corpo do bloco tem uma meia-volta fixa de 180° no modelo que o arquivo de máquina não expressa (rotação 0).
- **O fosso de cinza** que a chaminé alimenta já existia (`ashpit`); sem fonte de fumaça no port, a cadeia só roda em testes até haver geradores que emitam fumaça (caldeiras e queimadores).
