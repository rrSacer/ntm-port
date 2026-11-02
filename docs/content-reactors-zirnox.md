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

Uma escrita recusada guarda uma pendência por slot. Na leitura seguinte, se o
valor do host continua igual ao último confirmado, preserva o resultado pendente
da simulação. Se mudou por ação do jogador/funil, o novo valor vence e a pendência
daquele slot é descartada; os outros slots sempre são lidos. Crafters agregados
usam o valor desejado como base da reconciliação, sem repetir o delta da máquina.
Dados de tela recusados também são retentados; retornar ao valor já confirmado
limpa a pendência, sem bloquear um save estável na sequência A→B recusado→A. `native_save` aguarda as escritas
pendentes e só marca um JSON como salvo se `save_machine` devolver sucesso.
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
| Salvamento do nível estacionado/comparadores | Valem as limitações gerais já documentadas em `docs/host-dimensoes.md`. |

## Verificação após revisão independente

As correções R1–R7 estão em commits próprios. A ponte tem testes adicionais de
escrita parcial, retirada/substituição de itens e entradas de funil em máquinas
anteriores ao ZIRNOX (firebox e assembler agregado), além do cenário exato da
barra retirada durante a recusa. A sequência de tela A→B recusado→A tem save
aceito em ticks saudáveis. Blindagem cobre distância 20,5 e distância inteira;
redstone cobre as 98 células da superfície, incluindo `core+[2,3,2]`.

Base: `main` `df645f68`, motor `18eab7ab41e767d9e21dc4079cb2f21dbe116c95`,
upstream congelado `2050c4e0f`. Build **release**, com `SKIP_UPSTREAM_CHECK=1`
somente para esse snapshot; pacote refeito com o empacotador do mesmo build.

| Grupo | Resultado |
| --- | --- |
| Rust NTM | 327 unitários + 26 integração passaram; 2 doctests ignorados. |
| Clippy NTM | `--workspace -- -D warnings`, passou. |
| Clippy nativo transformado | Passou; somente `clippy::collapsible_if` isentado, diferença já conhecida do transformador upstream. |
| Python conteúdo | 218 testes passaram com recursos 26.3 explícitos. |
| Build nativo | Cargo release em 7min45s, 27 injeções; pacote 9.103.885 bytes. |
| ZIRNOX nativo | 105 verificações: 52 montagem, 2 dimensão, 6 conversões, 1 captura, 6 reabertura, 35 falha, 3 solo. |
| Pilha nativa | 26 montagem + 25 reabertura + 31 destruição + 1 precondição de ar positivo; main também passou as mesmas 26 verificações de montagem. |
| Dimensões | 6 montagem + 4 retomada funcional + 2 idle + 1 idle-clean, em mundos separados; todos passaram. |

A suíte inclui 12 testes de modelo ZIRNOX e 13 de servidor. Não há injeção de
estado de máquina nem `life`: o reator é colocado com seu item, barras e
recipientes entram pela tela, os dutos são tipados com o identificador, e
`/tick sprint` avança a simulação normal.

Resultados pós-revisão de ZIRNOX:

- 41.337 mB de superhotsteam em tanques externos, com consumo de água.
- Jogador confirmado no Nether forçado, `life` do Overworld 2.691→4.533.
- Li→trítio, Th232→combustível de tório e ZFB→esgotada por reação natural.
- Após encerrar/reabrir, `life` 43.042→43.569; mover pelo cursor conserva a vida.
- Falha destrói quatro marcadores, deixa destroço em chamas e `waste_earth`. Cerca e baú são preservados; tronco e tábua são convertidos.
- Baseline de criações zero; delta exato por tipo `[20,10,10,10,20,2]`, total 72. Sobreviventes novos `[20,10,10,10,14,1]`, total 65; os quatro tipos não destrutivos conservam os 50 itens esperados.
- Solo coberto: 8/8 amostras de `waste_earth` e 8/8 de `waste_mycelium` viram terra por random tick. O roteiro confirma `minecraft:random_tick_speed=512` antes de acelerar e restaura 3 depois.

SHA-256 cliente: `1e0d8b2250d83f3ed2ef8b1094b4445ca63e4659b5d11f415ee1ff35a86454b2`.
SHA-256 `dist/ntm.hmod`: `eb96b19246c029df3be847d25c1ae068f88b177940a9b48cc9b00635dbcab631`.

O driver `tools/test-game/run-native-reactors.py` cria mundo, configuração/cache e
Xvfb descartáveis; desliga controle físico só nesse diretório e encerra os
processos no final. `dims` executa também `idle` e `idle-clean`, cada um em mundo
novo sem chunks forçados. O cenário pile repõe ar pela tela antes da captura e
exige ar positivo. Não houve instalação, merge, push nem uso de mundo do jogador.

```sh
agent-run env QUEUE=game python3 tools/test-game/run-native-reactors.py zirnox \
  --binary /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/zirnox/native-target/release/horizon-client \
  --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 \
  --scratch-root /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/zirnox
```

Trocar `zirnox` por `pile` ou `dims`. Para comparar só a montagem térmica com a
main, usar `pile --setup-only`, o cliente de
`.agent-tmp/migracao-upstream/target/release/horizon-client` e
`--bundle /mnt/ssd120/sacer/horizon-ntm/ntm-port/dist/ntm.hmod`.

O Xvfb registra o aviso conhecido de TAA indisponível e usa o fallback; TAA não
é validado por este ensaio. O detector exclui somente esse aviso conhecido.


O fixture térmico da pilha foi corrigido em commits próprios, com autorização do
coordenador. A asserção antiga podia reconectar ar abaixo do equilíbrio
ventilado e observar aquecimento saudável. Exigir ar zero sem inserir os
controles também reprovou na própria main: o calor chegou a 789,84 com ar 12 e
falhou no tick seguinte. O fixture final insere os controles existentes enquanto
o ar residual dissipa, prepara o identificador real, extrai os controles,
aguarda calor em [150,400] e reconecta imediatamente. Falha explicitamente em
120 s sem cumprir a preparação ou se passa de 400 antes de detectar ar. O
limiar de falha da máquina continua estritamente >800. Mantém `b > a`, `d < h1`
e a janela de resfriamento de oito segundos.

Mesmos scripts, mundos novos e mesma configuração, sem compilação concorrente:

| Binário/pacote | Sem ar: a | Sem ar: b | Ar retornou: h1 | Após 8 s: d |
| --- | ---: | ---: | ---: | ---: |
| Main `df645f68` | 22,4612154166 | 168,0672008398 | 174,7426131791 | 44,1464398975 |
| Branch corrigida | 22,4612401205 | 168,0672269737 | 144,6971612115 | 44,1415975343 |

Ambos zeraram o ar em 61,55 s e atingiram a janela quente em 3,95 s. Retorno de
ar: 995, calor abaixo de 400 em ambos. A main usou os hashes originais de cliente
`e01fe940ff37a2e83f880ef2da74515c35020e60924c640a1a0b2cf7fa022893`
e pacote `d8cd6303742230d743e04220ac1da6af8a3f3bfacbf80a237dafe7b93870e130`.
A branch usou os hashes pós-revisão acima. Nenhuma regra de produção da pilha
foi alterada.


Dimensões: os dois fornos novos receberam 68.306 e 56.644 HE das máquinas
retomadas. `idle` com leitor e `idle-clean` sem leitor fecharam/salvaram seus
níveis em 17 s, cada qual em mundo novo sem chunks forçados. Todos os processos
do cliente e Xvfb foram encerrados.

Evidências pós-revisão em `.agent-tmp/zirnox/`, relativamente à raiz do projeto:

| Arquivo/pasta | Conteúdo aceito |
| --- | --- |
| `validation-review.json` | Índice agregado pós-revisão, hashes, contagens, comparação térmica e ensaios reprovados. |
| `zirnox-5x3_q3w_/` | 105 verificações, sete fases e capturas; criações por tipo contra baseline. |
| `pile-1uc2za2z/` | Main: 26 verificações do fixture reforçado com hashes originais. |
| `pile-lp2k66x2/` | Branch: 26 montagem + 25 reabertura + 31 destruição e precondição de ar positivo. |
| `dims-jq0sd1rp/` | Montagem e retomada funcional com fornos novos. |
| `dims-idle-hw0wvb_f/`, `dims-idle-clean-npe2c_0p/` | Mundos independentes, fechamento/salvamento em 17 s. |
| `review-core-tests.log`, `review-core-clippy.log`, `review-python-tests.log`, `review-native-build.log`, `review-native-clippy.log`, `review-content-pack.log` | Suíte, Clippy, build e pacote pós-revisão. |

Os ensaios reprovados foram preservados. O fixture original da branch voltou a
reconectar abaixo do equilíbrio (124,47→128,95), enquanto a main original
reconectou em 160,27 e esfriou para 129,12. A comparação final usa o fixture
reforçado idêntico aprovado nas duas versões, sem mudar o runtime. A primeira
prova de solo usou o nome antigo da gamerule, rejeitado pelo comando; a fase foi
retomada no mesmo binário/pacote após corrigir o nome e verificar o valor. Os
registros aceitos distinguem esses ensaios dos resultados finais.
