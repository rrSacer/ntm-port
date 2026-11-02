# Chicago Pile MK2

Runtime da pile MK2 na branch `pile-mk2`. A pile legada não faz parte desta frente. Referências: `ItemPileRodMK2.EnumPileRod`, `TileEntityPileCore`, `TileEntityPileLoader`, `TileEntityPileVent`, `TileEntityPileControl` e `BlockPile{,Brick,Device}`.

## Montagem e uso

Monte um paralelepípedo **maciço** de `pile_brick` e use uma broca manual em uma face horizontal, fora das arestas. O ponto clicado vira o núcleo. A altura e a largura detectadas vão de 5 a 15. A sondagem original examina 15 blocos atrás da face, permitindo profundidade de 5 a **16**; esse detalhe foi preservado. Tijolos faltantes impedem a conversão. Os blocos internos usam os nove IDs R1 já existentes, com vínculo ao núcleo no block entity.

A broca em um bloco formado perfura um canal completo na direção oposta à face clicada. Canais paralelos ao eixo do núcleo são de combustível; perpendiculares são de ar; verticais são de controle. Não podem cruzar o núcleo, arestas ou outros canais. Usar a broca novamente na entrada, na mesma direção, desfaz o canal e ejeta suas barras. A broca também pode operar através do dispositivo ligado à entrada.

O loader fica fora da entrada de combustível, orientado para fora. Um clique com uma barra põe **uma** unidade no loader; outro clique inicia o ciclo. O flanco de subida da redstone, no lado externo, também inicia o ciclo. O avanço é de 1/7 por tick, seguido de cinco ticks de espera e retorno de 1/7. No início do retorno, a barra é empurrada pela fila do canal. O excedente sai na extremidade oposta como entidade de item. Um ciclo vazio não extrai barras: a retirada original ocorre empurrando outra barra, desfazendo o canal ou desmontando a pile. Como no Java, a barra ejetada perde o dado `depletion`.

O vent recebe AIR na face externa, em tanque de 4.000 mB, e enche o canal de ar até 1.000 mB. O controle fica sobre a entrada vertical superior: transmite o nível atual antes de mover até o alvo, em passos de 1/60 por tick. Redstone sobe o alvo para 1 (extração total) e desce para 0 (inserção total). Sem GUI; o controle pelo clique não inventa um novo modo de operação.

Quebrar um membro ou o núcleo desmonta a pile, devolvendo os membros ainda presentes a tijolos e ejetando as barras uma vez. A própria célula quebrada também volta a tijolo; o jogador quebra o tijolo em uma segunda ação. O hook de mudança disponível não distingue jogador, explosão e comando, portanto `/setblock` e `/fill` sobre um membro também são sobrescritos pela reversão, como no Java. Células descarregadas suspendem a simulação, sem serem tratadas como falha estrutural. O núcleo e cada membro usam os block entities/persistência existentes; membros órfãos voltam a tijolos quando a ausência do núcleo pode ser confirmada.

**Dimensões e chunks.** O host só consegue ejetar itens e explodir na dimensão que está em lugar (`native_explode` devolve `false` fora dela), e no fim do tick do servidor essa é a *última* dimensão que ticou (Overworld, Nether, End), não necessariamente a da pile ou a do jogador. A simulação não precisa do nível em lugar: blocos, redstone e block entities de uma dimensão aberta são acessados diretamente (ver `docs/host-dimensoes.md`). O host só troca de nível, de forma preguiçosa e com guarda que o restaura mesmo em erro, quando há algo a ejetar ou explodir (`pile_drop`, `pile_explode`), e por isso não zera a ociosidade do nível nem impede o fechamento de uma dimensão sem jogador. Uma dimensão que o host não alcança (fechada) **congela** a simulação, os dispositivos, a varredura de órfãos e a desmontagem (`pile_reachable`), sem perder barras: a quebra do núcleo nesse estado mantém o núcleo até a dimensão poder ser alcançada. Quebrar um membro com parte da pile em chunk descarregado desmonta a pile na hora, como o `breakBlock` do Java; as células descarregadas ficam para a varredura de órfãos. O estado do núcleo é gravado mesmo quando outra dimensão ticou por último (o defeito do host que impedia isso está corrigido, `docs/host-dimensoes.md`).

## Reação e calor

| Barra | Fonte/tick | Reação | Vida | Calor/nêutron | Produto |
|---|---:|---:|---:|---:|---|
| RA226BE, PO210BE | 1 | 0 | permanente | 0 | — |
| ZR | 0 | 0 | permanente | 0 | — |
| NU | 0 | 1 | 25.000 | 0,25 | PU239 |
| PU239 | 0 | 1 | 500 | 0,5 | RGP |
| RGP | 0 | 1 | 1.000 | 0,5 | WASTE |
| WASTE | 0 | 1 | permanente | 1,5 | — |
| THORIUM | 0 | 1 | 35.000 | 0,25 | THORIUM_FUEL |
| THORIUM_FUEL | 0 | 1 | 2.000 | 0,5 | WASTE |

Cada barra recebe o fluxo anterior dividido pelo comprimento inteiro do canal, incluindo posições vazias. `squirt(x)=sqrt(x+1/(x+2)²)-1/(x+2)` determina a reação. A depleção soma o **fluxo produzido** e transmuta ao atingir a vida, descartando a sobra. A multiplicação de calor usa o tipo anterior à transmutação.

O fluxo produzido é somado dentro de cada segmento vertical e propagado entre segmentos para o próximo tick. Segmentos de controle multiplicam o fluxo que os atravessa por `clamp(sum(control)/(depth-1),0,0.5)`; não bloqueiam a própria emissão das fontes. Foi preservada a peculiaridade de `getChannelVerticalIndex`: rotação de uma direção vertical conserva componentes X/Z zero, portanto todos os canais de controle caem no índice `left`. Segmentos de combustível são cadastrados antes e têm precedência se ocuparem esse mesmo índice. Canais de combustível perfurados no sentido contrário também conservam a indexação calculada a partir da sua própria direção. A rotação usa a [matriz original de ForgeDirection 1.7.10](https://github.com/MinecraftForge/MinecraftForge/blob/1.7.10/src/main/java/net/minecraftforge/common/util/ForgeDirection.java), verificada também em volume assimétrico.

Cada canal de ar resfria os canais de combustível a até uma linha vertical de distância por `heat *= 1-(air/1000)*0,05`; consome `ceil((air/1000)*5)` mB/tick. Depois se aplica a todos os canais de combustível a perda passiva `*0,999`, com mínimo de 20°C. A falha ocorre estritamente **acima de 800°C depois do resfriamento**. Como no Java (`destroy()` com `meltingDown`), só o **núcleo** volta a tijolo: o combustível é ejetado e a explosão nativa de força 15, com fogo, dano a blocos e entidades, roda no centro médio dos canais e no topo da pile contra os membros ainda de pé. Os sobreviventes são revertidos a tijolo pela varredura de órfãos no tick seguinte (a varredura pula o tick da falha). Se a explosão não puder ser aplicada (chunk descarregado), isso é registrado no log.

## Dependência de conteúdo aprovada

Entraram somente `hand_drill` e `hand_drill_desh`, com nomes e texturas originais. Ambas empilham uma unidade; a primeira tem 100 de durabilidade e a segunda tem durabilidade ilimitada. O desgaste na pile usa o mecanismo de ferramenta do motor. As duas receitas preservam o padrão `␠D / S␠ / ␠S`: aço Dura (`DURA`) e gravetos para a normal; desh e o grupo original polímero/baquelite para a desh. A receita desh reutiliza o `MachineResolver` existente para resolver `ANY_PLASTIC`, sem ampliar o perfil das receitas antigas. A aba criativa vem de `ItemTooling` (controlTab). Nenhuma identidade anterior do catálogo foi alterada.

## Limites nominais

- A rede de fluidos NTM atual só representa identidade e volume; **a pressão de 1 PU exigida pelo vent Java não é validada**. AIR e as capacidades/consumos reais são preservados, sem inventar um segundo sistema de pressão.
- O tipo `EntityBulletBaseMK4` e a configuração `pile_debris` ainda não existem no port. Não são emitidos os **15 projéteis de destroços**, seus rastros de fogo, trajetória de 200 ticks ou explosões secundárias de força 5. A falha térmica, desmontagem, ejeção e explosão principal são executadas.
- Os modelos TESR, fan/haste/pistão animados, connected textures, overlay de inspeção original, partículas customizadas `tower` e sons customizados de encaixe/ferrolho não foram portados nesta frente. Permanecem os cubos R1 e o estado nativo inspecionável; a interação com o núcleo fornece temperatura no chat.
- Integração OpenComputers/Redstone-over-Radio e os usos das brocas em outros blocos `IToolable`, inclusive remanescente de crafting, estão fora deste runtime.
- A prova de jogo desta frente é no Overworld e com o jogador no Nether (pile derretendo com o Nether ticando por último). Uma pile em dimensão fechada só tem teste de servidor.

## Verificação

O modelo tem testes de todas as barras, limiar de vida, formação em quatro orientações, rejeição de volume/canais inválidos, ejeção, propagação e atraso de fluxo, controle, resfriamento, falha e persistência. O servidor testa notificações atrasadas, interação do loader, desmontagem (inclusive quebra por jogador: tijolo na própria célula e resto revertido), redstone, vent, descarga de células e explosão.

**`float_roundtrip` (escopo real: o binário do cliente).** O `serde_json` padrão lê `f64` com um parser rápido que pode errar o último bit. Fluxo, calor e depleção das barras são `double` salvos em JSON; ao reabrir o mundo, alguns valores mudavam em 1 ULP e a simulação divergia da que foi salva (num teste descartável, 14.662 de 200.000 valores semelhantes voltaram diferentes; com a feature, nenhum, e `native_reopen_keeps_channel_reaction_and_depletion` falha sem ela). A feature está em `[workspace.dependencies]`; no pacote nativo o `Cargo.toml` gerado de `ntm-core` também a fixa, e por unificação de features todo `serde_json` ligado ao `horizon-client` passa a usar o parser exato (o motor já faz isso em `horizon-port-pack`). O custo medido na leitura dos JSON de dados do crate (1,35 MB) é desprezível (2,89 → 3,35 ms, carga única). Nenhum formato de escrita muda, então saves existentes são lidos igual ou melhor. Alternativa sem efeito global, não adotada: gravar os `f64` como `u64` via `f64::to_bits`.

**Redstone.** Loader e controle leem o sinal que a *própria célula* recebe (`native_redstone_many`, um lote por dimensão; o vent não lê redstone). Equivale ao `getIndirectPowerLevelTo` do Java: uma alavanca ou botão encostado no dispositivo, um bloco de redstone adjacente ou fio apontando para ele o acionam. **Desvio deliberado:** o Java olha só a direção de saída do dispositivo; aqui as outras 5 faces também contam (uma alavanca em cima do loader o aciona). O motor não expõe o sinal por direção (`Level::signal`) e não foi criado mecanismo novo. O flanco de subida inicia o loader; no controle, sobe ou desce o alvo.

### Resultados (base rebaseada em `main` @ `01f0e8e3`, upstream `2f35078b3`, motor `2ca67d71e`)

- `cargo test --workspace`: 300 testes unitários + 26 de integração, todos verdes; `cargo clippy --workspace -- -D warnings` limpo. Testes novos da revisão: loader lê só a própria célula, pile em dimensão inativa congela sem perder barras, quebra de membro com chunk descarregado desmonta na hora, meltdown reverte só o núcleo e a varredura reverte o resto, `air` salvo é limitado a 1000, enchimento do vent sem estouro.
- 216 testes Python do `tools/content-gen` (seed 42, com `HORIZON_RESOURCES` apontando para os recursos do cliente 26.3); validadores de conteúdo e receitas passaram.
- Build nativo release no pin atual com `SKIP_UPSTREAM_CHECK=1` (o upstream já tinha avançado para `8ee72368`; a prova foi feita no SHA congelado `2f35078b3`).
- **Prova em jogo descartável** (`tools/test-game/native_pile.py`, mundo novo, Xvfb mudo, sem injetar estado): 23 verificações na montagem, 25 na reabertura do processo e 31 na fase de destruição, todas verdes.
  - A broca desh forma a pile 5×5×5 (`formed_shape`) e perfura 5 canais: 2 de combustível, 1 de ventilação, 2 de controle. Loaders e controles são colocados como itens/blocos reais pelo gancho de face clicada.
  - Ar comprimido real: um barril de aço recebe `fluid_barrel_full__air` pela interface, o ar segue por dutos tipados AIR e enche o vent (4.000 mB) e o canal de ventilação (995 mB). Os pacotes `fluid_pack_full__*` não são despejáveis no motor, então o roteiro usa o barril.
  - **Alavancas reais**: os dois controles são alimentados por uma alavanca encostada na própria célula. Abertos: fluxo cruzado 7,24; fechados (alavancas desligadas): 0,19. No loader, com a barra de NU esperando, a alavanca desligada não carrega; ligada, carrega (`lever_loads_rod`).
  - Cinco barras de Ra226Be e cinco de Pu239 carregadas por cliques; transmutação natural PU239 → RGP sem atalho.
  - Ventilação: sem suprimento a temperatura sobe (67,7 → 104,1 °C); com ar reconectado cai (97,4 °C).
  - A broca no loader desfaz o canal e ejeta as barras.
  - Reabertura: identidade, geometria, canais, barras, depleção, calor, fluxo, nível de controle, ar e conteúdo de loader/vent/controle/barril continuam coerentes.
  - **Quebra por jogador** (`break` com o cliente, segunda pile 5×5×5): todas as 125 células viram `pile_brick`, inclusive a quebrada.
  - **Jogador no Nether** (Nether com chunk forçado, pile no Overworld): o Overworld deixa de ser o nível em lugar no fim do tick, mas a pile continua simulando e derrete com explosão e ejeção, e o nível do jogador é restaurado depois do gancho (`overworld_pile_melts_while_player_in_nether`).
  - **Falha térmica** (pile sem controles nem ar, Ra226Be + Pu239 carregados por cliques): o núcleo e todos os membros terminam como tijolo/ar/fogo, sem sobrar `pile_block`, com explosão e o registro `pile meltdown` no log.

### Pendências registradas

- Leitura de redstone por direção (`Level::signal`) exigiria um hook novo no motor; as 5 faces extras contam (desvio documentado acima).
- Índice dos núcleos: `pile_core_at` ainda varre as máquinas do mundo por chamada (custo medido irrelevante pelo revisor).
- Casos especiais em código compartilhado (`block_slots`, `fluid_ports`, filtro de `block_event`) devem virar entradas de tabela do tipo se surgir um terceiro caso.
- Chunk partido não foi provado em jogo (tem teste de servidor). Uma pile em dimensão fechada (nenhum nível aberto para ela) congela e só tem teste de servidor.
