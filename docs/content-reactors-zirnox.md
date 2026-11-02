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
| Controle | O sinal recebido pelas células superficiais da própria estrutura força ligado; sua descida desliga. O botão alterna sem sinal. Não se consulta o sinal recebido por um bloco vizinho para inferir o do dispositivo. |
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
fonte e permanece desligada.

## Persistência, dimensões e entradas malformadas

O núcleo guarda calor, pressão, controles e tanques no JSON de sua block entity.
As barras vivem nos slots nativos. Detritos, poção de radiação, transformação do
terreno e explosões pendentes vivem na persistência já existente do mundo,
independentes da sobrevivência do núcleo.

Uma escrita recusada de slots é retentada, mantendo o resultado da simulação;
não é marcada como concluída nem sobrescrita pela leitura antiga do tick seguinte.
Dados de tela recusados também são retentados. `native_save` aguarda as escritas
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
| `WasteEarth.canSustainPlant` | Blocos de terra usam o suporte a plantas do template vanilla; falta a restrição Java a plantas Cave. |
| Salvamento do nível estacionado/comparadores | Valem as limitações gerais já documentadas em `docs/host-dimensoes.md`. |

## Verificação

Base: `main` `df645f68`, motor `18eab7ab41e767d9e21dc4079cb2f21dbe116c95`,
upstream congelado `2050c4e0f`. Build **release**, com `SKIP_UPSTREAM_CHECK=1`
somente para esse snapshot; o pacote foi refeito com o empacotador do mesmo build.

| Grupo | Resultado |
| --- | --- |
| Rust NTM | 319 unitários + 26 de integração, todos passaram; 2 doctests ignorados. |
| Clippy NTM | `--workspace -- -D warnings`, passou. |
| Clippy nativo transformado | Passou; somente `clippy::collapsible_if` isentado, como na validação anterior do motor (o transformador remove comentários do upstream). |
| Python conteúdo | 218 testes, todos passaram, com `HORIZON_RESOURCES` apontando para os recursos 26.3. |
| Build nativo final | Cargo release em 8min03s; conteúdo reempacotado, 9.103.867 bytes. |
| ZIRNOX nativo | 95 verificações: 50 montagem, 2 dimensão, 6 conversões, 1 captura, 6 reabertura, 30 falha. |
| Pilha nativa | 23 operação + 25 reabertura + 31 destruição; mais uma precondição de ar positivo. Roteiro original intacto. |
| Dimensões | 6 montagem + 4 reabertura funcional + 2 idle + 1 idle-clean. Roteiro original intacto. |

A suíte inclui 11 testes de modelo ZIRNOX e oito de servidor, cobrindo reação,
vizinhança, breeding, peculiaridade de água/vapor, limites de falha, controle,
portas, radiação por vida, recusa de gravação, retomada estrita e dimensões abertas
ou fechadas. A extração condicional futura tem uma função pura testada.

O driver reproduzível `tools/test-game/run-native-reactors.py` cria diretórios
novos para mundo/configuração/cache e um Xvfb próprio, usa apenas comandos normais,
cliques e recipientes e encerra os processos no final. Desliga o controle físico
apenas nesse diretório descartável; não altera configurações do usuário.

```sh
agent-run env QUEUE=game python3 tools/test-game/run-native-reactors.py zirnox \
  --binary /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/zirnox/native-target/release/horizon-client \
  --resources /home/sacer/Games/horizon/.local/horizon-client/26.3 \
  --scratch-root /mnt/ssd120/sacer/horizon-ntm/.agent-tmp/zirnox
```

Trocar `zirnox` por `pile` ou `dims` para as regressões. `dims` executa também
`idle` e `idle-clean`, cada qual em um mundo novo sem chunks forçados; `pile`
repõe ar pelo contêiner antes da captura, exige ar
positivo e mantém o roteiro original sem alterações.

SHA-256 do cliente: `c37fb64d16a345629449a7e37884a4038cac6bf07cd8866ef0ec4e76ea009a05`.
SHA-256 de `dist/ntm.hmod`: `4913a9f343001e38ffc35e67f77cdc11f97fc9d8daca3fbed590d21292474791`.


A prova ZIRNOX não injeta estado de máquina nem `life`. O reator é colocado com
seu item; as 24 barras e recipientes entram pela tela, os dutos são tipados com
o identificador e o tempo avança por `/tick sprint` normal. Resultados medidos:

- 38.837 mB de superhotsteam em tanques externos, com redução dos reservatórios de água.
- Com o jogador confirmado no Nether forçado, `life` do Overworld avança de 2.520 para 4.320.
- Li→trítio, Th232→combustível de tório e ZFB→esgotada por reação natural.
- Após salvar, encerrar e reabrir, `life` avança estritamente de 42.943 para 43.473; mover a barra pelo cursor preserva a vida.
- Uma segunda máquina sem refrigerante falha: destroço em chamas, quatro marcadores destruídos pela explosão, grama transformada em `waste_earth` e 64 detritos dos seis tipos ainda ativos.
- Na regressão de dimensões, as duas máquinas retomadas carregam fornos novos (68.306 e 56.644 HE). Sem chunks forçados, o Nether fecha e salva em 17 s com leitor e também sem leitor.

Evidências ficam em `.agent-tmp/zirnox/` relativamente à raiz do projeto:

| Arquivo/pasta | Conteúdo |
| --- | --- |
| `validation.json` | Índice agregado, contagens, hashes, pins e ensaios reprovados. |
| `zirnox-dcpmxt70/` | `report.json`, seis fases `native-zirnox-*.json` e sete capturas PNG. |
| `pile-nhqiu8wb/` | Relatórios de montagem, captura, reabertura e destruição; ar positivo antes da captura. |
| `dims-vhoibzxt/` | Montagem/reabertura com fontes e consumidores reais. |
| `dims-idle-dt65izrb/`, `dims-idle-clean-j6pviyt0/` | Fechamento/salvamento em mundos independentes. |
| `core-tests.log`, `core-clippy.log`, `python-tests.log`, `native-build.log`, `native-clippy.log`, `content-pack.log` | Logs de código e empacotamento. |

A primeira execução da pilha reprovou a janela térmica de oito segundos
(126,30→129,11 com ar); a repetição isolada passou o critério original
(129,68→128,95). A causa da primeira reprovação não foi atribuída. O primeiro
ensaio de idle foi descartado por manter o Nether forçado do setup; os ensaios
aceitos usam mundos novos. Esses registros foram mantidos no índice agregado.

O Xvfb emite o aviso conhecido de TAA indisponível e usa seu fallback. Esse aviso
é registrado e excluído somente do detector de falhas de conteúdo; TAA não faz
parte desta prova. Os clientes/Xvfb foram encerrados. Não houve instalação,
merge, push ou uso de mundo do jogador.
