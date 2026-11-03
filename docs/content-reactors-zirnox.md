# ZIRNOX no servidor nativo

A branch `zirnox` implementa o reator, reaproveitando as redes de fluidos,
block entities, menus, multiblocos, explosão e contaminação já existentes no NTM
ou no Horizon. O checkout e o pin do motor permanecem intactos. O bundle inclui uma ponte local mínima para concluir updates de células por meio do `Sim` upstream, registrada em `docs/horizon-current.md`.

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
inserção por funil. A política condicional compartilhada de
`canExtractItem(slot,stack,side)` consulta `zirnox::can_extract(slot,item)` nas
extrações por automação: barras ativas ficam; itens sem combustível ativo saem
dos slots 0..23. Isso também conserva dentro do reator o combustível ativo
produzido pelo breeding. Retirada manual continua permitida. Os slots 0..23 são
`Slot` vanilla no Java: a inserção manual (clique, tecla numérica, drag) aceita
qualquer item, uma unidade por slot; só a automação (`isItemValidForSlot`) e o
shift-click (`transferStackInSlot`, via `quick_move`) ficam restritos às barras.
Recipientes 24..27
só são acessíveis pela tela, como os slots acessíveis Java. Ver o
[contrato do hook e das origens Manual/Automation](native-slot-policy.md).

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
| Botões de tela/receita | Intenções guardam a geração e permanecem pendentes até o passo aceitar; recusa não perde nem duplica o clique. Limitação nominal do produtor: pacote antigo de uma janela ainda validada só por posição pode ser carimbado com o BE novo, antes de entrar na fila. Ver o ticket de identidade de janela abaixo. |
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

## Auditoria de gerações e mineração conjunta

Uma identidade é `(dimensão, posição, geração)`, acompanhada pelo estado esperado.
Quebras e recolocações são consumidas antes dos retries. O contador monotônico é
persistido; o limite `u64::MAX` congela a célula em vez de permitir ABA por overflow.
Uma ausência positiva grava a `Removal` no KV antes de testar disponibilidade do
resto do grupo. `None` significa inacessível e conserva o estado, sem ejeção.

| Tipo / produtor | Precondição | Geração diferente / retry |
| --- | --- | --- |
| `MachineCommit` e grupo da pilha | Geração do modelo/JSON confirmado, bloco e todos os slots observados | Recusa tudo, nenhum setter parcial; nova simulação usa o inventário observado. |
| Células lidas/consumidas pela escavadeira e `Drill` (well, pumpjack, fracker) | Estado completo, geração observada, chunk carregado, nível visitável e snapshot completo do BE/itens | Preflight recusa a operação inteira. Leituras de petróleo também participam quando o sorteio não esgota o depósito; não fica produto de uma leitura obsoleta. |
| `WorldEffect::Blocks` | Guarda por célula: geração e ID esperado; o preflight final lê estado completo e BE real | Descarta escrita obsoleta sem sobrescrever a nova geração. Se ainda é a geração condenada, retém também itens inseridos depois do commit da falha: substituição e drops NBT publicam atomicamente. A captura é persistida antes da publicação e descartada em recusa. Escritas legadas sem guarda não são reaplicadas. |
| `WorldEffect::Items` / `EncodedItems` | Origem confirmada do grupo e, para conteúdo minerado, célula antiga | Efeito físico da geração antiga: entrega uma vez, mesmo com ocupante novo. NBT completo dos itens é preservado pelo codec existente do motor. |
| `WorldEffect::PileExplosion` | Origem confirmada da pilha | A explosão commitada ocorre uma vez, com origem/potência antigas; pode destruir blocos novos no raio. |
| Fila de bomba ainda não admitida | `Queued.origin` da geração consumida; número persistente da explosão | Continua após commit. Antes do commit, a intenção antiga é cancelada. Após admissão vale o journal existente do motor. |
| Falha ZIRNOX, detritos e waste | Origem da geração que confirmou a falha; cursores/entidades persistentes | São consequências físicas antigas e continuam. A escrita direta do invólucro/destroço permanece condicionada às guardas de `Blocks`. Tuplas antigas de explosão ainda são lidas como efeitos físicos legados. |
| Intenção de detonador/pavio | Geração da bomba e componentes observados | Quebra cancela; recolocação com o mesmo ID/peças não reaproveita o pedido. Vínculo persistente do detonador por posição permanece. |
| Botões de tela/receita | Geração entregue pelo hook; protegida na fila e no consumo da intenção | Descarta intenção enfileirada obsoleta. **Limitação de motor no produtor:** a janela valida posição/existência/alcance, não a identidade histórica do BE; pacote em trânsito pode chegar depois de uma recolocação do mesmo ID. |
| Edição manual da pilha / reparo Stirling | Reconciliação dos fatos antes do clique, geração e commit do grupo/BE | Não edita uma geração quebrada; mão só é debitada depois da confirmação. |
| Efeitos transitórios locais (`tell`, `log`, `hurt`, `effect`, `show`) | Guardas das origens confirmadas; liberação síncrona depois do commit | Não sobrevivem como escrita tardia; guardas obsoletas impedem a liberação. |
| Sync de BE e retry pós-publicação | Estado/JSON da geração publicada; snapshot completo da célula para retry | Nunca repete crédito ou remoção. Nova quebra/recolocação cancela o sync; snapshot divergente é descartado. |
| `Removal` / tombstone | Geração removida confirmada, conteúdo e indicador de slots já pertencentes ao motor | Persistência imediata fora do rollback e do `all()` do grupo. Ocupante novo não substitui conteúdo antigo; inacessibilidade apenas espera. |
| Preparação de retenção antes da mineração | Snapshot completo da célula/BE original e geração, gravados no KV antes do consumo | Fonte intacta na mesma geração: descarta sem spawn. Fonte consumida ou caso ambíguo: replay físico. Fonte inacessível ou geração esgotada: congela. Recusa restaura o snapshot e elimina a preparação. |
| Recibo aplicado / `world_saved` | Origem e efeito confirmados, com a mesma guarda de célula quando é uma escrita | Reabertura limpa não repete efeito confirmado. Replay de escrita obsoleta é descartado; replay físico conserva o conteúdo antigo. |

A retenção de inventários minerados usa preparação por célula no KV. O BE da
máquina, as células consumidas e os spawns publicam juntos; em seguida a preparação
vira recibo aplicado. Uma recusa do journal impede o consumo. A recuperação
compara estado, geração e snapshot completo do BE: fonte original intacta
continua dona dos itens e não gera spawn; fonte consumida permite replay. O teste
de crash após commit simula célula salva com a entidade ainda não salva; o replay
restaura um spawn e a reabertura limpa seguinte não o repete. A janela de crash/I/O
agora também inclui a preparação anterior ao commit, com duplicação possível no
caso ambíguo. Ver a pendência nominal de save conjunto mundo/KV.

A fronteira de publicação das células é explícita. Primeiro são validados todos
os BEs e todas as células, inclusive mudanças externas entre a leitura e o commit.
Depois todos os slots/JSON e as células são publicados sob acesso exclusivo ao
`Game`, com operações previamente validadas. Só após a última publicação a ponte
`ntm_committed_updates` chama as regras existentes do `Sim`: `onPlace`, vizinhos,
comparadores e shapes indiretos. Luz, heightmap, malha e alterações de chunks usam
o mesmo `World::set` do caminho vanilla. BEs removidos têm inventário retido sem
`preRemoveSideEffects`, evitando uma segunda geração de drops. Isso também vale
para toda substituição estrutural adiada (pilha/destroço): itens novos vindos de
tela, funil, comando ou outro mod são lidos no preflight final e saem uma vez como
drops da geração antiga, no mesmo commit da substituição. Propriedades visuais
como `honey_level` não mudam a identidade; o snapshot completo é comparado sob
acesso exclusivo na publicação. Uma falha posterior
é log e retry de sync; jamais recusa ou rollback do produto já publicado.
Células `moving_piston`, cujo bloco carregado vive em `Ticks` fora do snapshot de
BE, aguardam o `finalTick` vanilla antes de serem consumidas.

| Máquina / chamada Java | Flags modernas / conclusão | Diferença do caminho anterior |
| --- | --- | --- |
| Escavadeira: `func_147480_a(..., false)`, `setBlockToAir`, `setBlock(barricade)` | Publicação sem side effects, seguida dos updates equivalentes à flag 3; destruição emite evento 2001 e `block_destroy`, sem loot duplicado | O `strict` anterior suprimia vizinhos/shapes. Esta rodada corrige a fidelidade ao Java. |
| `TileEntityOilDrillBase.tryDrill`: `setBlock(oil_pipe)` | Mesma flag 3, retendo o BE antigo e seu inventário | Tubo, crédito e estado agora publicam juntos; vizinhos/shapes são notificados. |
| Well/pumpjack/fracker: `setBlock(ore_oil_empty)` e oil spots | Mesma flag 3; todos os destinos e leituras relevantes participam do preflight | Depósito não some depois de um crédito recusado e um crédito não sobrevive à mudança do depósito. |

A varredura dos produtores de `set_blocks`/`mine_at` com crédito encontrou a
escavadeira e o caminho `Drill` compartilhado por well/pumpjack/fracker. Bombas,
pilha, invólucro ZIRNOX e waste são efeitos estruturais/físicos, sem crédito de
mineração. Não há outro cortador/bomba de fluido do mundo implementado nesta ponte.
As redes de energia/calor/fluido continuam depois dos passos confirmados.

A API `native_close_menu(uuid)` existente só fecha `Slots::Native`, não os menus
de BE das máquinas. Por decisão do coordenador, o NTM não acrescenta um protocolo
ou hook novo para fechar essas janelas nesta rodada. A pendência nominal é
[validar janela por identidade do BE, como `stillValid` vanilla](/root/.traycer/epics/5f50e725-564a-4cb7-848c-7bc5799a5749/artifacts/zirnox-menu-identidade/index.md).
A fila/consumo das intenções de botão está validada; o produtor conserva essa
limitação no pacote em trânsito entre quebra e recolocação do mesmo ID.

## Exclusões e aproximações nominais

| Fonte/API | Situação neste port |
| --- | --- |
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

## Diagnóstico da cerca e isolamento do teste de waste

A cena original foi repetida com `NTM_TRACE_EFFECTS=1`, sem alterar RNG,
slots ou estado interno. Em `zirnox-5ejtxm3l`, a explosão única ocorreu no tick
860, centro `[115,104,6]`, potência 12; seu plano não incluía nenhuma das quatro
amostras de madeira (`wood_targets=[]`). No tick 876, um detrito tipo 5, idade 16,
posição `[129.447724,101.124849,8.253124]`, removeu cerca, baú e tronco em
`[130,101,7..9]`. O trace da mudança de cada bloco coincide com o impacto. A tábua
restante virou `waste_planks` no tick 920. Logo, a reprovação era interferência
física dos detritos no teste de classificação do waste.

A repetição `zirnox-rgrxxana` passou na mesma cena: uma explosão, centro e potência
iguais, tick 766, sem madeira no plano e sem impacto nas amostras. Nas rodadas
históricas, as 50 direções horizontais não destrutivas persistidas permitem
reconstruir unicamente o seed e as 72 velocidades iniciais, com erro angular
numérico inferior a `2,1e-15`:

| Cena | Tick de criação do modelo | Seed | Sobreviventes por tipo |
| --- | ---: | ---: | --- |
| `zirnox-rj5f4434`, passou | 17664 | 17400911311430943377 | 20,10,10,10,11,2 |
| `zirnox-5x3_q3w_`, passou | 18323 | 14638207360112391339 | 20,10,10,10,13,1 |
| `zirnox-mft6c84m`, falhou na cerca | 18262 | 7991530272019292422 | 20,10,10,10,12,0 |

Todas têm a origem `[115,101,6]`, explosão em `[115,104,6]`, potência 12 e criação
por tipo `[20,10,10,10,20,2]`. Os ticks variam com o tempo das interações do roteiro,
e por isso também variam os seeds e trajetórias. O fixture `4eaf3673` fixa a
orientação de montagem e coloca as amostras a x=24 relativo ao núcleo, atrás de
uma parede de obsidiana. As quatro expectativas permanecem exatas: cerca e baú
inalterados, tronco em `waste_log`, tábua em `waste_planks`. Não foi preciso alterar
a explosão nem as regras físicas.

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

Base `df645f68`; runtime final `e920c2f3` e fixture `4eaf3673`. Motor
`18eab7ab41e767d9e21dc4079cb2f21dbe116c95`, upstream congelado `2050c4e0f`.
`SKIP_UPSTREAM_CHECK=1` somente para esse snapshot. Checkout e pin do motor/main
permanecem intactos; a ponte local de updates está registrada em `horizon-current.md`.

R8–R10 falham nos três cenários exatos do harness sobre `439ff430` e passam
sobre o código final. A suíte inclui troca por outro ID e pelo mesmo ID com
inventário novo, pedidos de nuke e pavio recusados antes da recolocação,
explosão já confirmada antes da admissão e remoção positiva com outro membro
inacessível. A mineração tem contraprova nas três famílias: o código anterior
credita minério/petróleo apesar da recusa da célula; a publicação conjunta conserva
produto, depósito e inventário retido. Essa contraprova de mineração usa uma cópia
isolada com `resource:false`, que restaura apenas a fronteira anterior de efeitos
e mantém as mesmas reproduções; não é um build integral do HEAD antigo. O teste
nativo compara o caminho novo com `set_block` vanilla flag 3, em armazenamento real, luz/heightmap, inventário NBT
completo, observador e circuito de redstone, sem drops extras.

| Verificação | Resultado |
| --- | --- |
| Rust NTM | 369 unitários + 26 integração; 2 doctests ignorados. |
| Clippy NTM | `--workspace --all-targets -- -D warnings`, passou. |
| Clippy nativo transformado | Passou; única isenção `clippy::collapsible_if` do transformador upstream. |
| Python conteúdo | 218 testes, recursos 26.3 explícitos. |
| Harness R8–R10 | 3/3 passam; os mesmos 3/3 falham no HEAD anterior. |
| Equivalência nativa da mineração | 1 teste passou, incluindo observador/redstone. |
| Build nativo | Release em 6min25s, 31 injeções/6 módulos; pacote 9.103.885 bytes. |
| ZIRNOX | 110 verificações, incluindo Nether forçado, conversões, retomada estrita, falha e solo. |
| Pilha | 83 verificações, incluindo ar positivo, retomada e destruição. |
| Dimensões | 13 verificações; idle e idle-clean em mundos novos separados. |
| Bombas | 49 verificações: Little Boy por detonador e alavanca; dinamite com pavio por detonador/redstone. |
| Mineração física | 18 verificações: escavadeira, perfuratriz e fracker, com petróleo real e retenção de 7 diamantes de um baú minerado. |

As **273 verificações nativas** passaram no mesmo cliente/pacote. As provas usam
montagem, tela, itens e fluidos reais; não injetam vida, estado de máquina ou pavio.
`/tick sprint` avança a simulação normal. `/ntmbomba` configura e confirma o raio 12
pelo comando nativo de operador. Após encerramento limpo, os sete mundos têm zero
remoções, preparações de retenção, efeitos pendentes e recibos aplicados no KV.

ZIRNOX produziu **39679 mB** de vapor em tanques externos. A vida avançou
**2589→4389** no Overworld enquanto o jogador estava no Nether
forçado, e **42958→43488** após reabrir. Li→trítio,
Th232→combustível de tório e ZFB→esgotada ocorrem pela reação natural. A falha cria
destroço e uma explosão, transforma o terreno e conserva as expectativas exatas
de waste nas amostras protegidas. Criações por tipo: **[20, 10, 10, 10, 20, 2]**, total 72;
sobreviventes novos: **[20, 10, 10, 10, 14, 0]**. Os quatro tipos não destrutivos mantêm os
50 esperados. Solos cobertos viram terra: 8/8 amostras por solo, gamerule restaurada.

```text
cliente sha256 55cfa5eef190b337a05627d4f2b1e906461da94ac22c6dd91eda54fab88fd78e
ntm.hmod sha256 eb96b19246c029df3be847d25c1ae068f88b177940a9b48cc9b00635dbcab631
```

Dimensões: os fornos novos receberam **68306 e 56644 HE** das máquinas
retomadas. Os níveis sem chunks forçados fecharam/salvaram em **17 s** com
leitor e **17 s** sem leitor, em mundos separados. O roteiro de bombas mantém
`count('Pavio aceso.') == 1`. Os testes de geração verificam separadamente o
pavio sem reset em retry e o cancelamento do pedido anterior à recolocação.

### Fixture térmico e comparação histórica aprovada

`f7e806ec` prepara ar zero com controles existentes, extrai controles, aguarda
calor em [150,400] e reconecta imediatamente. Reprova em timeout de 120 s ou acima
de 400 antes do ar voltar. Mantém `b>a`, `d<h1` e janela de 8 s; falha da pilha
continua estritamente >800. Nenhuma regra térmica de produção foi alterada.

| Execução | a sem ar | b sem ar | h1 ar de volta | d após 8 s |
| --- | ---: | ---: | ---: | ---: |
| Main df645f68, aprovada anteriormente | 22,4612154166 | 168,0672008398 | 174,7426131791 | 44,1464398975 |
| Branch anterior, comparação aprovada | 22,4612401205 | 168,0672269737 | 144,6971612115 | 44,1415975343 |
| Branch, runtime final e920c2f3 | 22.4612401205 | 168.0672269737 | 164.9369800370 | 44.1448595439 |

Main e branch anteriores passaram as mesmas 26 verificações de montagem, com
hashes originais da main: cliente
`e01fe940ff37a2e83f880ef2da74515c35020e60924c640a1a0b2cf7fa022893`, pacote
`d8cd6303742230d743e04220ac1da6af8a3f3bfacbf80a237dafe7b93870e130`.
Os estados térmicos capturados também aquecem sem ar e esfriam com ar no
harness do revisor. Essa comparação histórica não usa o novo binário.

### Evidências reproduzíveis

Índice completo: `.agent-tmp/zirnox/validation-generation.json`, na raiz do projeto.
O agregador exige o mesmo cliente/pacote em todos os cenários e verifica os quatro
journals vazios após fechamento limpo.

| Pasta em `.agent-tmp/zirnox/` | Verificações |
| --- | --- |
| `mining-3el41xhf` | mining: 18; fases setup. |
| `zirnox-wm3o9s9k` | zirnox: 110; fases setup, inactive, convert, capture-state, check-state, failure, soils. |
| `pile-n6x9i5kq` | pile: 83; fases setup, capture-state, check-state, destroy. |
| `dims-vjoyzngf` | dims: 10; fases setup, capture-state, check-state. |
| `dims-idle-el6g0zdl` | dims-idle: 2; fases idle. |
| `dims-idle-clean-2ka6uszg` | dims-idle-clean: 1; fases idle-clean. |
| `bombs-m1r6hcko` | bombs: 49; fases setup. |

Capturas no diretório do ZIRNOX: `native-zirnox-loaded.png`,
`native-zirnox-life-tooltip.png`, `native-zirnox-reopened.png`,
`native-zirnox-failure.png` e `native-zirnox-soils.png`.

Logs em `.agent-tmp/zirnox/`: `generation-core-tests.log`,
`generation-core-clippy.log`, `generation-python.log`, `generation-native-build.log`,
`generation-native-equivalence.log`, `generation-native-clippy.log`,
`generation-content-pack.log`, `generation-probe-final.log`, `generation-baseline.log`,
`mining-baseline-machine_{excavator,well,fracking_tower}.log` e
`generation-{mining,bombs,zirnox,pile,dims}-game.log`. O diagnóstico físico usa
`zirnox-5ejtxm3l/client-first.log` e `trajectory-reconstruction.jsonl`.
As contraprovas falham nas asserções de conservação/geração, não em compilação.

O driver `tools/test-game/run-native-reactors.py` isola mundo, opções/cache e Xvfb;
desliga controle físico só no diretório descartável e encerra seus processos.
`dims` cria automaticamente outros dois mundos para idle e idle-clean, sem chunks
forçados. O detector exclui somente o aviso conhecido de TAA indisponível;
essa renderização não foi validada. Nenhum mundo do jogador foi usado.

```sh
agent-run env QUEUE=game NTM_TRACE_EFFECTS=1 python3 tools/test-game/run-native-reactors.py zirnox \
  --binary /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/zirnox/native-target/release/horizon-client \
  --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 \
  --scratch-root /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/zirnox
```

Trocar `zirnox` por `pile`, `dims`, `bombs` ou `mining`. `--failure-only` é apenas
um diagnóstico e não substitui a aceitação completa. A comparação histórica com
main usa `pile --setup-only`, cliente de `.agent-tmp/migracao-upstream/target/`
e `--bundle /mnt/ssd120/sacer/horizon-ntm/ntm-port/dist/ntm.hmod`.
