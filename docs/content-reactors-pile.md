# Chicago Pile MK2

Runtime em implementação na branch `pile-mk2`. A pile legada não faz parte desta frente. Referências: `ItemPileRodMK2.EnumPileRod`, `TileEntityPileCore`, `TileEntityPileLoader`, `TileEntityPileVent`, `TileEntityPileControl` e `BlockPile{,Brick,Device}`.

## Montagem e uso

Monte um paralelepípedo **maciço** de `pile_brick` e use uma broca manual em uma face horizontal, fora das arestas. O ponto clicado vira o núcleo. A altura e a largura detectadas vão de 5 a 15. A sondagem original examina 15 blocos atrás da face, permitindo profundidade de 5 a **16**; esse detalhe foi preservado. Tijolos faltantes impedem a conversão. Os blocos internos usam os nove IDs R1 já existentes, com vínculo ao núcleo no block entity.

A broca em um bloco formado perfura um canal completo na direção oposta à face clicada. Canais paralelos ao eixo do núcleo são de combustível; perpendiculares são de ar; verticais são de controle. Não podem cruzar o núcleo, arestas ou outros canais. Usar a broca novamente na entrada, na mesma direção, desfaz o canal e ejeta suas barras. A broca também pode operar através do dispositivo ligado à entrada.

O loader fica fora da entrada de combustível, orientado para fora. Um clique com uma barra põe **uma** unidade no loader; outro clique inicia o ciclo. O flanco de subida da redstone, no lado externo, também inicia o ciclo. O avanço é de 1/7 por tick, seguido de cinco ticks de espera e retorno de 1/7. No início do retorno, a barra é empurrada pela fila do canal. O excedente sai na extremidade oposta como entidade de item. Um ciclo vazio não extrai barras: a retirada original ocorre empurrando outra barra, desfazendo o canal ou desmontando a pile. Como no Java, a barra ejetada perde o dado `depletion`.

O vent recebe AIR na face externa, em tanque de 4.000 mB, e enche o canal de ar até 1.000 mB. O controle fica sobre a entrada vertical superior: transmite o nível atual antes de mover até o alvo, em passos de 1/60 por tick. Redstone sobe o alvo para 1 (extração total) e desce para 0 (inserção total). Sem GUI; o controle pelo clique não inventa um novo modo de operação.

Quebrar um membro ou o núcleo desmonta a pile, devolvendo os membros ainda presentes a tijolos e ejetando as barras uma vez. A própria célula quebrada também volta a tijolo; o jogador quebra o tijolo em uma segunda ação. O hook de mudança disponível não distingue jogador, explosão e comando, portanto `/setblock` e `/fill` sobre um membro também são sobrescritos pela reversão, como no Java. Células descarregadas suspendem a simulação, sem serem tratadas como falha estrutural. O núcleo e cada membro usam os block entities/persistência existentes; membros órfãos voltam a tijolos quando a ausência do núcleo pode ser confirmada.

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

Cada canal de ar resfria os canais de combustível a até uma linha vertical de distância por `heat *= 1-(air/1000)*0,05`; consome `ceil((air/1000)*5)` mB/tick. Depois se aplica a todos os canais de combustível a perda passiva `*0,999`, com mínimo de 20°C. A falha ocorre estritamente **acima de 800°C depois do resfriamento**. Desmonta-se a pile, ejeta-se o combustível e chama-se a explosão nativa de força 15, com fogo, dano a blocos e entidades, no centro médio dos canais e no topo da pile.

## Dependência de conteúdo aprovada

Entraram somente `hand_drill` e `hand_drill_desh`, com nomes e texturas originais. Ambas empilham uma unidade; a primeira tem 100 de durabilidade e a segunda tem durabilidade ilimitada. O desgaste na pile usa o mecanismo de ferramenta do motor. As duas receitas preservam o padrão `␠D / S␠ / ␠S`: aço Dura (`DURA`) e gravetos para a normal; desh e o grupo original polímero/baquelite para a desh. A receita desh reutiliza o `MachineResolver` existente para resolver `ANY_PLASTIC`, sem ampliar o perfil das receitas antigas. A aba criativa vem de `ItemTooling` (controlTab). Nenhuma identidade anterior do catálogo foi alterada.

## Limites nominais

- A rede de fluidos NTM atual só representa identidade e volume; **a pressão de 1 PU exigida pelo vent Java não é validada**. AIR e as capacidades/consumos reais são preservados, sem inventar um segundo sistema de pressão.
- O tipo `EntityBulletBaseMK4` e a configuração `pile_debris` ainda não existem no port. Não são emitidos os **15 projéteis de destroços**, seus rastros de fogo, trajetória de 200 ticks ou explosões secundárias de força 5. A falha térmica, desmontagem, ejeção e explosão principal são executadas.
- Os modelos TESR, fan/haste/pistão animados, connected textures, overlay de inspeção original, partículas customizadas `tower` e sons customizados de encaixe/ferrolho não foram portados nesta frente. Permanecem os cubos R1 e o estado nativo inspecionável; a interação com o núcleo fornece temperatura no chat.
- Integração OpenComputers/Redstone-over-Radio e os usos das brocas em outros blocos `IToolable`, inclusive remanescente de crafting, estão fora deste runtime.
- A prova de jogo desta frente é no Overworld. A persistência de BEs fora da dimensão ativa mantém a limitação preexistente do host; não se afirma aceitação multidimensional completa.

## Verificação

O modelo tem testes de todas as barras, limiar de vida, formação em quatro orientações, rejeição de volume/canais inválidos, ejeção, propagação e atraso de fluxo, controle, resfriamento, falha e persistência. O servidor testa notificações atrasadas, interação do loader, desmontagem, redstone, vent, descarga de células e explosão. A leitura de JSON usa `float_roundtrip`, pois a configuração anterior mudava alguns valores de fluxo em um ULP ao reabrir.

Resultados iniciais: 290 testes unitários + 26 de integração do `ntm-core`; 216 testes Python; validadores de conteúdo e receitas passaram. Build nativo, Clippy, prova descartável e rebase sobre a atualização do upstream ainda em andamento.
