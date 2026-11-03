# ZIRNOX no servidor nativo

A branch `zirnox` implementa o reator, reaproveitando as redes de fluidos,
block entities, menus, multiblocos, explosão e contaminação já existentes no NTM
ou no Horizon. A implementação não altera o motor.

## Contrato de simulação

A fonte de dados é `content/machine-recipes/reactors_simple.json`, copiada pelo
`tools/machine-gen/build.py` para `crates/ntm-core/data/reactors_simple.json`.
`zirnox.rs` lê diretamente as 11 barras/conversões, as 24 listas de vizinhos e
as capacidades. Não há uma segunda extração Java nem outra tabela de combustíveis.

| Regra | Comportamento |
| --- | --- |
| Inventário | 24 barras e quatro slots de recipientes: CO₂ 24→26, água 25→27. Uma barra por slot. |
| Vida | `minecraft:custom_data.life`, inteiro. A vida acompanha a barra na tela/inventário; a conversão usa **`life > max_life`** e cria o produto sem dados anteriores. Outros campos de custom data sobrevivem enquanto a barra permanece ativa. |
| Vizinhança | Cada barra não breeding se consome uma vez por si própria e por vizinho ativo. Th232/Li só avançam por vizinhos ativos. Slots são processados de 0 a 23; uma conversão anterior altera a avaliação dos seguintes. |
| Breeding | Th232→combustível de tório; Li→trítio. Barras breeding não contam como vizinho ativo. |
| Pressão | `2*CO₂ + int(float(heat)*(float(CO₂)/16000))`, antes do resfriamento. Operações `f32` na mesma ordem Java. |
| Água e vapor | Com `0 < heat < 100000`, água/CO₂ presentes e vapor livre: a partir de heat>10256, calcula o ciclo de superhotsteam e reduz o calor pela pressão. O ciclo **não é limitado pela água restante nem pelo espaço do vapor**; os tanques são limitados separadamente. Sem essas condições, subtrai 10 de calor. |
| CO₂ | Não é consumido pela produção de vapor na fonte Java. A válvula libera até 1000 mB por clique. |
| Limites | Falha após o resfriamento se pressão>100000 ou calor>100000; igualdade não falha. Calor igual a 100000 não entra no resfriamento. |
| Controle | O sinal recebido em toda a superfície da caixa 5×5×5 força ligado; sua descida desliga. O botão alterna sem sinal. A varredura inclui os cantos sem invólucro, como no Java; não consulta células externas à caixa para inferir o sinal. |
| Portas | Quatro faces externas, lateral ±2, alturas +1 e +3, com rotação. Recebem água/CO₂ e enviam superhotsteam. O núcleo e as outras faces não são portas. Não há produção de HE. |
| Recipientes | Carregam um por tick com as funções existentes de recipientes; aguardam espaço no tanque e na saída vazia. |

A tela original e seus 28 slots são reaproveitados, com calor, pressão, ligado,
três tanques, temperatura em °C, pressão em bar e o valor de redstone. O tooltip
da barra mostra vida consumida e percentual sem usar dano vanilla. Os limites/filtros valem no menu nativo e na
inserção por funil.

A radiação de uma barra no inventário usa `HazardModifierFuelRadiation`:
`base + (alvo-base)*(life/maxLife)^0,4`, preservando a ordem float/double Java e
quantizando ao milliRAD/s da camada de radiação existente. Os 11 alvos são
exportados como dados numéricos em `hazards.json`. Servidor e tooltip usam a mesma
função; `NativeCarried.custom_data`, já disponível no motor, fornece `life` sem
uma API nova. Até o lítio inicialmente não radioativo passa a emitir ao breeding.

## Falha e destroços

A falha limpa os 28 slots, troca o núcleo por `zirnox_destroyed`, conserva sua base
5×2×5 e remove a parte alta do reator original. A explosão nativa tem potência
12, fogo e dano/destruição reais, na dimensão do reator. Os avisos diferidos da
própria troca não apagam o novo estado. Uma explosão recusada fica pendente na
persistência do mundo para nova tentativa.

A quebra do destroço usa a loot table nativa para os seis drops Java: 6 concretos,
4 tubos de aço, 2 grades, 6 detritos metálicos, 2 grafite e 4 fallout.

A resistência do invólucro passou a 60 e a do destroço a 480, inclusive os proxies invisíveis específicos que usam o
mesmo mecanismo barrier dos multiblocos existentes, equivalentes aos
valores Java `setResistance(100)` e `setResistance(800)` após a escala 3/5 de
`getExplosionResistance`. A resistência fallback 10 anterior deixava a própria
explosão destruir o destroço. O destroço tem dureza 100, como a fonte.

Os 72 detritos são simulados no estado do mundo: 2 trocadores, 20 concreto,
20 vigas, 10 elementos, 10 grafite e 10 fragmentos. A simulação guarda velocidades,
gravidade 0,04, amortecimento 0,85/reflexão −0,5, tempo de vida de cada tipo e a
regra destrutiva dos tipos concreto/trocador ascendentes. Elementos/grafite
renovam a contaminação de 60 s, amplificadores 7/4, numa vizinhança de 2,5 blocos.
A dose reproduz `HbmPotion.radiation`: `(amplificador+1)*0,05 RAD` por tick.
A criação real de cada tipo incrementa `debris_created[0..5]` na persistência do
mundo. A prova lê uma linha de base antes da falha e exige o delta exato
`[20,10,10,10,20,2]`, independentemente de colisões posteriores; exige também os
50 sobreviventes novos dos quatro tipos sem remoção destrutiva. Não confunde a
contagem de sobreviventes com os 72 criados.

O padrão Java `dialEnablePermaScrap=true` mantém esses detritos permanentemente;
o tempo de vida só expira quando essa propriedade salva é falsa.

O destroço emite 500000 RAD por tick em chamas, 75000 apagado, divididos pela
resistência acumulada no percurso de um bloco e pelo quadrado da distância aos
olhos do jogador. O host lê a resistência real dos blocos do motor na dimensão
pedida. Em chamas e a menos de cinco blocos, aplica 2 de dano de fogo. A chama
pode cessar com probabilidade 1/5000 por tick; espuma acima dos blocos do destroço
pode apagá-la com probabilidade 1/25 na atualização periódica. Seu estado de
fogo é salvo de forma consistente; a divergência `fire`/`onFire` da leitura e
escrita Java não é reproduzida.

`ExplosionNukeGeneric.waste(...,35)` transforma o terreno numa esfera irregular
(`d² < 612 + random(122)`), processada em lotes de 4096 posições. Não é uma
explosão nuclear nem a chuva de fallout das bombas. As transformações incluem
terra contaminada/micélio, madeira, trinitita, argila, portas e os minérios da
fonte, com a chance padrão 1/250 de schrabídio. `waste_earth` e `waste_mycelium`
foram declarados com as texturas Java, drop de terra e luz do micélio; este aplica
a contaminação ao caminhar. A propagação do micélio é desligada por padrão na
fonte e permanece desligada. Os dois solos recebem random ticks pelo hook nativo
existente: pouca luz acima (<4) e opacidade acima (>2) convertem o solo em terra.
A leitura usa a luz/opacidade e o escurecimento do nível que está executando o
hook (mesma origem de luz de `native_light`), e a troca usa `Behaviours::set_block`.
`RadiationConfig.cleanupDeadDirt` é `false` no perfil Java padrão
(`RADWORLD_03_regrow`); este port segue esse padrão sem criar outro toggle.
A conversão de cogumelo acima para `mush` permanece excluída: o bloco tem regras
próprias e será item futuro de catálogo K4, conforme decisão do coordenador.

A seleção de madeira do `wasteDest` usa um catálogo próprio de equivalentes
opacos (troncos, tábuas e blocos inteiros de madeira). Cercas, portões e baús
permanecem intactos. A classificação compartilhada das crateras não foi alterada.

## Persistência, dimensões e entradas malformadas

O núcleo guarda calor, pressão, controles e tanques no JSON de sua block entity.
As barras vivem nos slots nativos. Detritos, poção de radiação, transformação do
terreno e explosões pendentes vivem na persistência já existente do mundo,
independentes da sobrevivência do núcleo.

Cada passo local começa com um snapshot completo da máquina. `sync_in`, consumo
de recipientes, trabalho e `sync_out` pertencem ao mesmo passo especulativo.
`commit_machine` compara os slots observados e publica todos os slots, dados de
menu e JSON juntos. O host nativo valida antes de mutar, sob acesso exclusivo ao
`Game`; não existe fallback para setters parciais. Uma recusa restaura o snapshot
e descarta efeitos. No tick seguinte o inventário atual do jogador/funil é lido
novamente. Hosts sem commit atômico congelam a máquina.

Loader, núcleo e membros de cada pilha formam um grupo transacional; perfuração
que ejeta barras também confirma o novo núcleo antes da ejeção. Um membro
inacessível mantém o grupo congelado até todos os BEs necessários voltarem.
Quebra confirmada por evento `Broken` ou bloco diferente em chunk acessível gera
um tombstone; inacessibilidade nunca equivale a quebra. O tombstone conserva o
estado completo no KV mesmo se o BE já sumiu e a dimensão fechar antes da ejeção.

Efeitos de blocos, drops e explosão da pilha ficam numa fila persistida. Recusas
mantêm o cursor; efeitos aceitos não se repetem em memória. Recibos dos efeitos
aplicados permanecem no KV até o hook existente `world_saved(wait=true)` confirmar
o save de chunks e entidades. Reabertura limpa após essa confirmação não repete
a ejeção. A janela entre save do mundo e cursor KV pode repetir efeitos após um
crash/falha de I/O; a escolha autorizada conserva o replay, com risco de duplicar,
sem confirmar prematuramente um spawn ainda não salvo. A solução definitiva
exige um commit conjunto de mundo/KV ou efeitos persistidos pelo próprio motor.

Redes de energia, calor e fluidos rodam depois dos passos confirmados. Uma recusa
de save dessas trocas conserva o estado em memória para regravar. Solar/caldeira
transfere apenas calor/fluido. Na foundry, derretimento/produção local de itens
é transacional; calor e transferências de metal fundido só operam depois sobre
estados confirmados, sem tocar slots. Materialização de pseudo-fluido no fim da
cascata de centrifugação também pertence ao passo local. As fases térmicas agora
usam o estado confirmado do passo anterior quando necessário (latência de um
tick); as redes não podem receber produtos de um consumo recusado.

Entradas fora do tick foram auditadas nesta correção:

| Entrada | Fronteira de confirmação |
| --- | --- |
| Detonador e acendimento de pavio | O clique enfileira uma intenção com os slots observados. O próximo passo local relê componentes e confirma slots/estado antes de liberar detonação, som ou partículas. Recusa mantém a intenção; alteração das peças cancela com feedback. Há até um tick de latência. Mesmo jogador/item/intenção usa a janela de um tick de `native_block_claim`, incluindo o fallback cliente bloco→ar; pedido pendente/pavio aceso é no-op e nunca repete a confirmação nem reinicia o timer. |
| Botões de tela/receita | Intenções permanecem pendentes até o passo aceitar; recusa não perde nem duplica o clique. |
| Inserção manual no loader da pilha | Grupo do loader/núcleo confirma antes de solicitar o débito da barra na mão, no mesmo acesso exclusivo ao `Game`. |
| Broca em pilha existente/ejeção por quebra | Transação do grupo; geração quebrada não recebe edição manual. Ejeção adiada por tombstone. |
| Engrenagem de reparo da Stirling | Confirma o BE antes de consumir a engrenagem da mão no mesmo `Game`; recusa conserva ambos os estados. |
| Alavanca de turbina, espelho/mirror_tool e identificador de fluido | Apenas configuração/ligação; não consomem itens nem emitem entidades. Estado é confirmado no próximo passo. |
| Comandos de bomba | Registro nativo adicionado em `8c2bead8` usando os hooks existentes `register_commands`/`perform_command`, com operador nível 2. Configuração/status não consomem recursos. `/ntmbomba teste` permanece a ação administrativa explícita sem insumo autorizada; nenhuma implementação paralela do comando. |
| Formação inicial da pilha pela broca | **Pendência nominal de motor:** cria BEs novos e muda os tijolos antes da primeira confirmação do JSON; desgasta a broca depois da montagem. Não move barras/recipientes. Criação dos BEs, estado e durabilidade da mão não possuem um commit conjunto; save/crash nessa fronteira pode deixar a formação sem metadados ou com desgaste divergente. Edições de pilhas existentes usam o grupo atômico acima. |

A instrumentação nativa confirmou mão principal no vínculo em tick398, seu
fallback `Item.use` no399 e o clique explícito no403. O cliente prevê PASS para
`Item.useOn` do mod e segue para `remote_use_hands`; o filtro consome esse
fallback. O roteiro confirma que apenas vincular não acende nem detona.

Um save limpo entre `Broken` e o próximo tick registra a remoção sem simular nem
ejetar após o save. A persistência também guarda se o motor já possui os slots
físicos, evitando soltá-los de novo quando só restam metais/barras internos a
restituir. Um BE recém-colocado salvo antes do primeiro tick é adotado a partir
do bloco e de seus slots reais na reabertura.

Valores de saves são limitados antes da aritmética: tanques, life, calor, cursores
e detritos. Incrementos usam saturação quando apropriado.

Máquinas em dimensões abertas estacionadas continuam rodando e gravando por
`Host::entity`. Explosão, sons e partículas passam por `in_place`/`visit_level`.
Dimensões fechadas congelam máquinas/detritos; nenhum acesso abre um nível.

## Exclusões e aproximações nominais

| Fonte/API | Situação neste port |
| --- | --- |
| `canExtractItem(slot,stack,side)` | **Pendência de motor aceita provisoriamente pelo coordenador.** O pin só permite listas estáticas de saída. Funis podem inserir barras em 0..23; sua extração automática fica desabilitada, inclusive de barras ativas. Retirada manual funciona. Recipientes 24..27 só pela tela, como os slots acessíveis Java. A função pura `zirnox::can_extract(slot,item)` já expressa e testa a regra definitiva: apenas itens convertidos saem dos 24 slots. Falta um hook condicional em `ModContainer`/`can_take`. |
| `EntityZirnoxDebris` e seu renderer/interação | Não existe registro de entidade customizada equivalente no bundle atual. As regras são simuladas e persistidas; o desenho usa partículas nativas. Não há malha giratória, caixa de colisão por tipo nem coleta por clique direito. Colisões são aproximadas por voxel em vez de `moveEntity`/raytrace das caixas Java. |
| `EntityLivingBase` | Dano e contaminação do host atual alcançam jogadores; a mesma consulta em mobs não está exposta. Sem radiação/dano em mobs. |
| `gas_meltdown`, `foam_layer` | Não estão no catálogo. A regra sorteia e conta emissões, exibidas por fumaça nativa; não cria um bloco fictício nem simula sua propagação. `block_foam` existente apaga o destroço. |
| `AuxParticlePacketNT.rbmkflame`, som `hbm:block.rbmk_explosion` | Substituídos por chama/fumaça e sons de fogo/explosão nativos. |
| `SatelliteRayScan`, conquista `achZIRNOXBoom`, `radMark`/elementais e `BossSpawnHandler.markFBI` | Sistemas correspondentes ausentes; não emitem evento/conquista/flag/spawn. |
| `checkTilt(CONFIG,true)` | Os toggles de gravidade e o perfil 528 estão desligados na configuração Java padrão; este port mantém esse perfil, sem criar uma configuração de inclinação paralela. |
| OpenComputers, EnergyControl e redstone-over-radio | Integrações de mods externos sem equivalente instalado. GUI e redstone locais funcionam. |
| `waste` em chunks descarregados | A API existente acessa somente blocos carregados; esses pontos são ignorados, sem carregar/gerar chunks para o efeito. A transformação das posições carregadas segue o limite/ordem da fonte. |
| `WasteEarth.updateTick`: cogumelo→`mush` | Exclusão de catálogo K4 aceita pelo coordenador; `mush` não está declarado e possui regras próprias fora do ZIRNOX. Escurecimento está implementado; `cleanupDeadDirt=false` segue o padrão Java. |
| `WasteEarth.canSustainPlant` | Blocos de terra usam o suporte a plantas do template vanilla; falta a restrição Java a plantas Cave. |
| Crash/I/O entre `spawn_item`/save do mundo e cursor KV | **Pendência de motor autorizada.** Recibos só são descartados após `world_saved(wait=true)`; antes da confirmação, o replay pode duplicar efeitos. Não há garantia de commit conjunto dos dois armazenamentos. Recusas, retries e reabertura limpa após cursor confirmado têm ejeção exatamente uma vez. |
| Salvamento do nível estacionado/comparadores | Valem as limitações gerais já documentadas em `docs/host-dimensoes.md`. |

## Verificação após revisão independente

R1 foi refeito em `2a80e3b0`, seguido por `f910e450`, `048e38ca` e
`ff755b15`: transação completa, proteção de geração do BE, retenção de entradas
até confirmação, entradas manuais e checkpoint de remoções. `8c2bead8` registra
o comando administrativo pelo hook nativo existente, com operador nível 2.
`f95897b1` deduplica uso nativo e torna pavio/pedido pendente idempotentes;
`f5cb2778` consome o fallback cliente bloco→ar.
A reconciliação por célula foi removida.

Os mesmos testes de conservação sobre `edb12e8e` falham nas quatro famílias:

| Máquina | Total no código anterior | Total fornecido |
| --- | --- | --- |
| ZIRNOX | 48.000 mB de água | 32.000 mB |
| Diesel | 48.000 mB de combustível | 32.000 mB |
| Assembler | 3 lingotes/placas equivalentes | 2 unidades |
| Firebox | 5.999 ticks de combustível + trabalho | 4.000 ticks |

A suíte nova passa **36 combinações**: quatro famílias × recusa total, parcial
ou após staging × retirada do jogador, incremento do mesmo item ou funil.
Conta tanques, recipientes, itens no host/jogador/funil e trabalho. Inclui os
casos exatos do revisor, conflito de `expected`, host sem commit atômico,
loader/core/ejeção, perfuração, tombstone inacessível, retry de ejeção parcial,
recibos e reabertura limpa, substituição por BE novo do mesmo tipo, intenção de
detonador recusada/cancelada e reparo com engrenagem recusado. Um save entre
quebra e tick conserva os metais internos sem repetir os slots já soltos pelo
motor. R2–R7 e o fixture `f7e806ec` foram aprovados na rechecagem anterior.

Base `df645f68`; runtime final `f5cb2778`. Motor
`18eab7ab41e767d9e21dc4079cb2f21dbe116c95`, upstream congelado `2050c4e0f`.
`SKIP_UPSTREAM_CHECK=1` somente para esse snapshot. Release e pacote da branch
refeitos juntos; nenhuma alteração no motor/main.

| Verificação | Resultado |
| --- | --- |
| Rust NTM | 349 unitários + 26 integração; 2 doctests ignorados. |
| Clippy NTM | `--workspace --all-targets -- -D warnings`, passou. |
| Clippy nativo transformado | Passou com a única isenção `clippy::collapsible_if` do transformador upstream. |
| Python conteúdo | 218 testes, recursos 26.3 explícitos. |
| Build nativo | Release em 6min09s, 30 injeções; pacote 9.103.885 bytes. |
| ZIRNOX | 109 verificações: montagem, Nether forçado, conversões, captura, reabertura, falha e solo. |
| Pilha | 83 verificações: montagem, ar positivo, reabertura e destruição. |
| Dimensões | 13 verificações; idle e idle-clean em mundos novos separados. |
| Bombas | 49 verificações: Little Boy com cinco peças reais por detonador e por alavanca; dinamite com pavio por detonador e redstone. Comando nativo confirma e relê o raio 12. |

As provas colocam itens e movimentam barras/recipientes/peças pela tela;
não injetam estado de máquina, vida ou pavio. `/tick sprint` avança a simulação
normal. Detonador registra solicitação antes da confirmação; peças não retornam
ao jogador/mundo. Após encerramento limpo, todos os cenários têm zero remoções,
efeitos pendentes e recibos da ponte de máquinas no KV.

ZIRNOX: **40187 mB** de vapor em tanques externos, com consumo de água;
vida **2610→4419** no Overworld com jogador no
Nether forçado; vida **42965→43497** após reabrir.
Li→trítio, Th232→combustível de tório e ZFB→esgotada ocorrem por reação natural. Falha deixa
destroço, explode marcadores e transforma terreno, preservando cerca/baú.
Criações por tipo contra baseline [0, 0, 0, 0, 0, 0]: **[20, 10, 10, 10, 20, 2]**,
total 72. Sobreviventes novos **[20, 10, 10, 10, 11, 2]**; os quatro tipos
não destrutivos mantêm os 50 esperados. Ambos os solos cobertos viram terra por
random tick, 8/8 amostras de cada; a gamerule é consultada e restaurada.

```text
cliente sha256 08fa0d441fa3523657806c8fc101d5676a000db7fb6dff13689e3ea984614c49
ntm.hmod sha256 eb96b19246c029df3be847d25c1ae068f88b177940a9b48cc9b00635dbcab631
```

Dimensões: os fornos novos receberam **68.306 e 56.644 HE** das máquinas
retomadas. Os níveis sem chunks forçados fecharam/salvaram em **17 s**, tanto
com leitor (`idle`) quanto sem leitor (`idle-clean`), em mundos distintos.

Bombas: o traço final registra vínculo Main401→fallback402 ignorado→clique417;
a dinamite registra vínculo Main1044→fallback1045 ignorado→clique1062, com
**um** `Pavio aceso.`. O estado consultado entre vínculo e clique tem pavio
vazio. A prova não confunde o fallback com uma segunda intenção do jogador.

### Fixture térmico e comparação histórica aprovada

`f7e806ec` prepara ar zero com controles existentes, extrai controles, aguarda
calor em [150,400] e reconecta imediatamente. Reprova em timeout de 120 s ou acima
de 400 antes do ar voltar. Mantém `b>a`, `d<h1` e janela de 8 s; falha da pilha
continua estritamente >800. Nenhuma regra térmica de produção foi alterada.

| Execução | a sem ar | b sem ar | h1 ar de volta | d após 8 s |
| --- | ---: | ---: | ---: | ---: |
| Main df645f68, aprovada anteriormente | 22,4612154166 | 168,0672008398 | 174,7426131791 | 44,1464398975 |
| Branch anterior, comparação aprovada | 22,4612401205 | 168,0672269737 | 144,6971612115 | 44,1415975343 |
| Branch, runtime transacional final | 22.4612401205 | 168.0672269737 | 157.2842962683 | 44.1436261768 |

Main e branch anteriores passaram as mesmas 26 verificações de montagem, com
hashes originais da main: cliente
`e01fe940ff37a2e83f880ef2da74515c35020e60924c640a1a0b2cf7fa022893`, pacote
`d8cd6303742230d743e04220ac1da6af8a3f3bfacbf80a237dafe7b93870e130`.
Os estados térmicos capturados também aquecem sem ar e esfriam com ar no
harness do revisor. Essa comparação histórica não usa o novo binário.

### Evidências reproduzíveis

Índice completo: `.agent-tmp/zirnox/validation-transaction.json`, na raiz do
projeto. O agregador exige o mesmo cliente/pacote em todos os cenários.

| Pasta em `.agent-tmp/zirnox/` | Verificações |
| --- | --- |
| `zirnox-rj5f4434` | zirnox: 109; fases setup, inactive, convert, capture-state, check-state, failure, soils. |
| `pile-sr66zl0d` | pile: 83; fases setup, capture-state, check-state, destroy. |
| `dims-sujmq7wd` | dims: 10; fases setup, capture-state, check-state. |
| `dims-idle-ru_wfq9m` | dims-idle: 2; fases idle. |
| `dims-idle-clean-qp6v8mh9` | dims-idle-clean: 1; fases idle-clean. |
| `bombs-wlaf75ji` | bombs: 49; fases setup. |

Capturas do ZIRNOX no diretório acima: `native-zirnox-loaded.png`,
`native-zirnox-life-tooltip.png` (tela/vida/indicadores),
`native-zirnox-reopened.png`, `native-zirnox-failure.png` e
`native-zirnox-soils.png`. A tela e o tooltip também foram inspecionados.

Logs: `detonator-tests.log`, `detonator-clippy.log`, `transaction-python.log`,
`transaction-native-build.log`, `transaction-native-clippy.log`,
`transaction-content-pack.log`, `atomic-probe.log` e
`conservation-baseline.log`. A contraprova antiga falha em conservação, não em
compilação. Ensaios preliminares reprovados permanecem nas pastas descartáveis.

O driver `tools/test-game/run-native-reactors.py` isola mundo, opções/cache e
Xvfb; desliga controle físico só no diretório descartável e encerra processos.
`dims` cria automaticamente outros dois mundos para idle e idle-clean, sem
chunks forçados. O detector exclui somente o aviso conhecido de TAA indisponível;
essa renderização não foi validada. Nenhum mundo do jogador foi usado.

```sh
agent-run env QUEUE=game python3 tools/test-game/run-native-reactors.py zirnox \
  --binary /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/zirnox/native-target/release/horizon-client \
  --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 \
  --scratch-root /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/zirnox
```

Trocar `zirnox` por `pile`, `dims` ou `bombs`. A comparação só de montagem com
main usa `pile --setup-only`, cliente de `.agent-tmp/migracao-upstream/target/`
e `--bundle /mnt/ssd120/sacer/horizon-ntm/ntm-port/dist/ntm.hmod`.
