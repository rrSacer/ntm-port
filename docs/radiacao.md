# Radiação básica do M1

Implementação de 2026-10-28, em Rust/WASM. Unidades `RAD` são as unidades de
jogabilidade do NTM. A fonte da extração é o checkout irmão `ntm-src`.

## Dados extraídos

`crates/ntm-core/data/hazards.json` cobre todos os ids de `content/ids.json`.
O M1 tinha 96 ids e 13 radioativos; desde a importação em massa (2026-10-29) são
~1780 ids, **219 radioativos e 96 taxas distintas**. Os testes derivam essas
contagens da tabela, em vez de fixá-las. `tools/hazard-gen/extract.py` regenera a tabela e confere
as constantes da fonte antes de escrever. Cada entrada positiva contém
`arquivo:linha`, valor em RAD/s por item e inteiro equivalente em milliRAD/s.
Zero significa ausência de registro `RADIATION` nessa seleção de conteúdo.
A tabela também registra amianto e hidroatividade de lítio; esses dois riscos
não são aplicados pela lógica M1.

| Forma | Urânio natural (RAD/s/item) | Tório-232 (RAD/s/item) |
| --- | ---: | ---: |
| Nugget | 0,035 | 0,010 |
| Lingote/minério | 0,350 | 0,100 |
| Tarugo | 0,175 | não está nos ids M1 |
| Pó | 1,050 | 0,300 |
| Bloco de armazenamento/cristal | 3,500 | 1,000 |

As constantes são `u = 0.35F`, `th232 = 0.1F`
(`src/main/java/com/hbm/hazard/HazardRegistry.java:86` e `:88`).
Os multiplicadores usados são os de `HazardRegistry.java:133`: nugget 0,1,
lingote/minério 1, tarugo 0,5, pó 3, bloco/cristal 10. **Não usar a razão de
massa de `MaterialShapes`**: `OreDictManager.java:838` e métodos seguintes
escolhem explicitamente os multiplicadores de hazard. Os registros de urânio e
tório são `inventory/OreDictManager.java:347` e `:351`; cristais têm registro
próprio em `hazard/HazardRegistry.java:485` e `:486`.

## Regras do NTM 1.7.10

Todos os caminhos desta seção são relativos a `ntm-src/src/main/java/com/hbm/`.

- `hazard/HazardSystem.java:194`: processa inventário principal e armadura.
  `hazard/type/HazardTypeRadiation.java:25`: multiplica o nível pelo tamanho da
  pilha, divide por 20 e contamina a cada tick. Portanto a soma é linear em
  quantidade: 64 lingotes de urânio emitem 22,4 RAD/s.
- `util/ContaminationUtil.java:269`: acumula primeiro a exposição ambiental;
  ignora criativo, jogadores nos primeiros 200 ticks e entidades imunes; aplica
  resistência antes de incrementar dose. `extprop/HbmLivingProps.java:87`
  soma e limita a dose a 0..2500 RAD. `:304` e `:331` salvam/carregam
  `hfr_radiation` no NBT.
- `util/ContaminationUtil.java:34`: fator de proteção `10^(-resistência)`.
  `handler/HazmatRegistry.java:201`: soma as quatro peças, cladding e +0,2 do
  Rad-X. `hazard/type/HazardTypeRadiation.java:27`: reacher altera a dose por
  uma função de redução (ou divide por 49 no modo 528).
- `handler/EntityEffectHandler.java:219`: abaixo de 200 RAD não há doença.
  Entre 200 e 399: náusea e fraqueza I; a partir de 400: lentidão I e fraqueza
  II; a partir de 600: lentidão/fraqueza III e veneno II; a partir de 800:
  veneno III e wither II. São sorteios por tick, com intervalos esperados de
  300, 500 ou 700 ticks; os detalhes estão em `:235` até `:258`.
  Em 1000 RAD (`:223`), aplica dano 1000 e zera a dose.
- **Não há decaimento natural da dose do jogador nesse caminho.** Tirar o
  material interrompe a entrada, mas conserva a dose. Reduções explícitas
  existem em `items/food/ItemPill.java:69` (-100), `potion/HbmPotion.java:126`
  e `tileentity/machine/TileEntityDecon.java:26` (-0,5). O decaimento de
  radiação ambiental em `handler/radiation/ChunkRadiationHandlerNT.java:252`
  pertence a outro sistema, não ao dosímetro do jogador.
- `handler/EntityEffectHandler.java:272`: o chunk também contamina. Não é o
  mesmo que estar carregando o item-bloco. `HazardTypeRadiation.java:48` não
  aplica radiação de entidade item largada por esse método.

## Recorte implementado

A dose é um inteiro milliRAD por UUID em `WorldState.radiation`, serializado
no mesmo save `w.<wid>` e journal da fundação. `#[serde(default)]` permite ler
saves anteriores sem o campo. Não há chave de dose adicional por mundo.
O limite por save é o da fundação (256 KiB). Para evitar serializar o mundo
inteiro a cada segundo de exposição, mudanças comuns de dose solicitam save
urgente no máximo a cada **100 ticks (5 s)**. Cruzar um limiar de doença
(200/400/600/800/1000 RAD) salva imediatamente. Há um flush da última dose
pendente mesmo depois de largar os itens. **Sair antes do próximo flush pode
perder até 5 s de dose**, janela aceita para o M1. `/ntmlogic save` também
permite salvar explicitamente, mas não é necessário para o teste de reentrada.

O servidor amostra o inventário aproximadamente uma vez por segundo, com
`execute store result score #count ntm_rad_m1 run clear @s #<tag> 0` (veja
"Amostragem por bits", abaixo). O zero **só conta**, não consome nada. É uma consulta ao
inventário inteiro, inclusive mão secundária, grade de craft e item no cursor, em vez de testar só a mão
selecionada. O client mod não lê o inventário.

O objetivo `ntm_rad_m1` é criado por `scoreboard objectives add … dummy` a
cada leitura: o erro de já existir é inofensivo e a consulta continua. Todos os
scores temporários são reescritos. A soma ponderada é codificada em oito
dígitos base 8, em nove células emprestadas (o último token confirma fim da
sonda). O servidor só aceita todos os dígitos mais a confirmação. Cada linha
que escreve token inclui `execute in <dim>`.

O módulo envia no máximo **16 linhas por tick**. Ele cede à sonda de
identidade e aos ticks em que saem transações de máquinas. Assim deixa
orçamento para as máquinas dentro do teto de 64 comandos/tick do servidor
integrado; a conta está abaixo. O valor wire satura em 16.777,215 RAD/s
(`radiation::MAX_RATE`).
Só são emprestados blocos sólidos estáveis da whitelist: não água, lava, ar,
cascalho, blocos com NBT ou máquinas.

As sondas usam o mecanismo compartilhado da fundação (`probes.rs`): células,
estado original e mundo são persistidos com checksum antes dos comandos.
A recuperação só repõe células que ainda são tokens no mundo correto. Uma
leitura radioativa tardia restaura células e é descartada; os comandos ainda
não enviados são cancelados. Não se aplica dose de uma sessão anterior.

Uma resposta confirmada integra a exposição **desde a amostra confirmada
anterior** (`radiation::exposure_ticks`), até `MAX_SAMPLE_GAP` (200 ticks, e
pelo menos dois períodos do modo). A volta de uma amostra (envio, cliente,
consumidor, leitura) torna o ciclo mais longo que o período, e muito mais num
PC carregado: integrar só o período subdoseava em proporção. Sem resposta
(prazo, `Incomplete`, tags desconhecidas), na carência e numa sessão nova,
o intervalo não é integrado: a próxima amostra vale só o próprio período.
Isso prefere subestimar exposição durante travamentos a continuar irradiando
um jogador que já largou os itens.

O prazo da sonda conta a partir do **ack** do cliente para a última leva de
linhas dela (`probes::ACTIVE_TICKS`, 100 ticks), não do envio: um cliente
que roda mais devagar que o servidor não perde amostras. Sem ack, ela espera
até `probes::UNACKED_TICKS` (1200) mais o tempo de envio. Largar a pilha zera a taxa na próxima
leitura confirmada; a dose permanece. Os primeiros 10 segundos após entrada
protegem o jogador, e criativo/espectador não recebem exposição.

A doença usa efeitos vanilla determinísticos de 5 s nos mesmos limiares do
NTM, renovados a cada leitura. A partir de 800 RAD também dá 1 ponto de dano
mágico por segundo; em 1000 RAD dá 1000 `generic_kill` e zera a dose. Essa é
uma aproximação deliberada dos sorteios/partículas/vômito do NTM. Efeitos de
outras origens não são limpos. Armaduras hazmat, reacher, medicamentos e
proteção de outros mods ainda não estão implementados, pois não existem no
conteúdo selecionado.

O HUD `hud` no client mod mostra taxa confirmada em RAD/s, dose em RAD e a
condição de espera/sonda sem confirmação. As cores usam os limiares do NTM.
Com mais de um jogador, o HUD avisa discretamente e a sondagem é suspensa:
o M1 inteiro suporta single player, com uma mailbox compartilhada. Comandos
precisam estar habilitados no mundo.

## Amostragem por bits (catálogo grande)

O desenho do M1 gastava 5 linhas por item radioativo. Com 219 itens, isso
dava ~1100 linhas e mais de 60 ticks por amostra. Agrupar por taxa exata (96
tags `ntm:rad/rate_N`) ainda custaria pelo menos 3 × 96 = 288 linhas. A
amostra agora soma **por bit da taxa**, sem aproximar nenhuma taxa:

    Σ count·taxa = Σ_K 2^K · count(itens cuja taxa tem o bit K)

- **Tags.** `content/data/ntm/tags/item/rad/bit_K.json` tem os ids de todos os
  itens cuja `radiation_millirad_s` tem o bit K ligado. Os ids são diretos,
  sem tags aninhadas, e só existem os bits presentes: hoje 27 (0–24, 26 e 28).
  O gerador do agente de importação (`hazard-gen`) as emite junto com as
  `rate_N`, que ficam para auditoria. `radiation_buckets.json` lista as duas.
  O teste `bit_tags_on_disk_match_the_table` confere que cada `bit_K` no disco
  é exatamente o derivado do `hazards.json`, sem bit a mais nem a menos, e que
  cada taxa se reconstrói dos seus bits.
- **Horner.** A soma vai do bit mais alto ao 0. Por posição há uma dobra
  (`*= #two`). Por bit presente há 4 linhas:
  1. `set #count -1`;
  2. `store result #count run clear @s #ntm:rad/bit_K 0`;
  3. marcar `#bad` se `#count` ficou em -1;
  4. `#total += #count`.
- **Tag ausente.** Uma linha que não é analisada (um `ntm.hmod` antigo, sem a
  tag) não grava nada: `#count` fica em -1, `#bad` vira 1 e o token de
  confirmação não é escrito. A amostra fica **sem confirmação** em vez de lida
  a menos.
- **32 bits.** Os scores do jogo são i32. A cada 4 bits, a soma parcial é
  limitada a 2^24 (`MAX_RATE + 1`). Como os termos não são negativos e os
  passos seguintes só dobram e somam, uma parcial que chegou lá garante uma
  soma final ≥ teto. Entre dois limites, a parcial cresce no máximo 2^4 vezes
  mais 15 contagens, abaixo de 2^31 para qualquer inventário com menos de
  2^26 itens. O mundo falso agora tem scores i32 com wrap, como o jogo; sem os
  limites, 8 ovos de balefire (300 000 RAD/s cada) dariam negativo.

**Bloqueio no Horizon (2026-10-29).** Em jogo, `clear @s #ntm:rad/bit_K 0`
dá "Unknown item tag". A causa é que `horizon-client/src/mod_content.rs` (regra
de `command-registries.json`) acrescenta os **ids** de itens e blocos dos mods,
mas não as **tags** dos mods, aos nomes que os comandos aceitam. Não é falta
das tags: elas estão no `.hmod` e na raiz montada
(`.horizon-cache/mod-content/data/ntm/tags/item/rad/`), e `#minecraft:…` de
vanilla funciona. A proteção acima segurou: a amostra ficou **sem confirmação**
(screenshot `img/radiacao-tag-desconhecida.png`), sem dose errada.

Há duas saídas, e as duas existem: a correção do motor em
`horizon-patches/mod-item-tags.diff` (sobre o upstream 9e4903787, verificada
em jogo; veja "Verificação em jogo") e a reserva automática por ids, abaixo,
para o Horizon sem patch.

**O mesmo vale para tags de bloco** (`execute if block … #ns:tag`): o parser
de predicados de bloco consulta `r.tags("minecraft:block")`, que vem de
`block_tags` do mesmo export, e o patch completa as duas listas. Isso não foi
testado em jogo, porque o `ntm.hmod` não tem tags de bloco próprias. Outros
registros com tags (entity_type, function etc.) **não** são cobertos pelo
patch; nenhum é usado pelo mod hoje.

**Conta** (teste `a_sample_fits_its_second`):

| Parte | Linhas |
| --- | ---: |
| Cabeçalho (objetivo, `#total`, `#two`, `#eight`, `#bad`) | 5 |
| Dobras: uma por posição abaixo do bit mais alto (28) | 28 |
| Bits presentes: 27 × 4 | 108 |
| Limites a cada 4 bits (28, 24, …, 4) | 7 |
| Criativo/espectador e teto final | 3 |
| Oito dígitos base 8: 8 × (2 + 8 + 1) | 88 |
| Confirmação: dois níveis (ok, ou tags desconhecidas) | 2 |
| **Total** | **241** |

- Com 16 linhas por tick, uma amostra leva **15 ticks**, dentro dos 20 entre
  amostras (`SAMPLE_TICKS`). Isso dá **1 amostra/s**, com média de 12
  linhas/tick do consumidor de 64.
- O custo cresce com o número de bits das taxas (no máximo 31), nunca com o
  número de itens. Um item novo com uma taxa nova não acrescenta linha, a não
  ser que ligue um bit inédito.
- A linha mais longa tem bem menos de 256 caracteres, e os ids nem aparecem
  nas linhas.

**Testes no mundo falso**, que executa as linhas reais e resolve `#tag` pelos
arquivos de `content/`:

- **`every_radioactive_item_counts_exactly`:** cada item radioativo (228 hoje), em duas
  pilhas, dá exatamente `n × taxa` (ou o teto).
- **`every_bucket_together_sums_exactly`:** um item de cada taxa < 100 RAD/s
  juntos, com e sem itens vanilla ao lado, dá a soma exata; todos juntos dão
  o teto.
- **`the_ceiling_is_exact_and_nothing_wraps`:**
  - `MAX_RATE - 1`, `MAX_RATE` e `MAX_RATE + 1` são lidos exatos;
  - o item mais quente em 1..=300, 2624 e 2^20 unidades dá o teto;
  - o catálogo inteiro em pilhas de 64 dá o teto.
- **`a_missing_tag_is_never_read_as_a_lower_rate`:** sem cada uma das 27 tags,
  ou sem todas, a amostra não é confirmada.
- **`samples_keep_their_cadence_beside_machines`:**
  - 9 a 11 amostras confirmadas em 10 s;
  - nunca mais de 16 linhas de radiação por chamada;
  - com o queimador recebendo 3 carvões no meio, o carvão é tirado e creditado
    exatamente e as amostras continuam;
  - toda chamada com linhas de transação cabe num tick do consumidor (≤ 64).

**Mutações:**

| Mutação | Teste que falha |
| --- | --- |
| sem os limites a cada 4 bits | `the_ceiling…` |
| sem a checagem `#bad` | `a_missing_tag…` |
| reset para 0 em vez de -1 | `a_missing_tag…` |
| sem as dobras nos bits ausentes | só a conta de linhas. Esses bits (25 e 27) estão acima do teto, então isso nunca muda um resultado |

## Reserva automática: amostra por ids

No Horizon oficial, sem `horizon-patches/mod-item-tags.diff`, os comandos não
conhecem as tags do mod. Para o mod funcionar assim mesmo, a amostra cai para
uma contagem **por id de item**.

- **Sinal.** A célula de confirmação tem dois níveis: `ok_level` (confirmada) e
  `ok_level + 4` (todas as linhas rodaram, mas alguma tag era desconhecida,
  `#bad = 1`). `radiation::read` separa `Rate`, `TagsUnknown` e `Incomplete`.
  Sem comandos ou com o cliente atrasado o resultado é `Incomplete`, que não
  conta para a troca.
- **Troca.** Depois de `TAG_MISSES` (3) amostras seguidas `TagsUnknown`, o modo
  vira `Ids`. O log registra a troca, e o HUD mostra "Modo compatível: leitura
  por ids a cada 3 s".
- **Volta.** O modo volta a `Tags` em toda entrada no mundo (o runtime é
  zerado). Em modo `Ids`, uma amostra por tags é tentada a cada
  `TAG_RETRY_TICKS` (5 min); se ela confirmar, o modo volta a `Tags` na hora.
- **Linhas por item** (4), cada uma com o seu caminho de storage
  `ntmlogic:rad r.i<k>`, todos removidos no início:
  1. `execute store result storage ntmlogic:rad r.i<k> int <taxa> run clear @s <id> 0`:
     a contagem já sai multiplicada pela taxa. O `int` satura, como o
     `(int)` do Java (`java_int` no Horizon), em vez de dar wrap.
  2. ler o caminho para `#count`;
  3. limitar `#count` a `MAX_RATE + 1`;
  4. `#total += #count`.

  Um limite de `#total` a cada 64 itens mantém a soma abaixo de 2^31. Um id que
  o jogo não tem (um `ntm.hmod` mais antigo) deixa o caminho ausente e conta 0.
  Isso é correto, porque ninguém o carrega, e ele nunca repete a contagem de
  outro item.
- **Cadência.** Com 228 itens radioativos, a amostra tem cerca de 1050 linhas,
  ~66 ticks a 16 por tick. O período (`ids_period`) é calculado do próprio
  número de linhas mais 6 ticks: ~3,6 s hoje, e cresce com o catálogo.
- **Dose.** Uma amostra confirmada vale os ticks desde a anterior (seu
  período, se a anterior não teve taxa), nos dois modos. A dose por segundo é
  a mesma nos dois modos; só a resolução muda.
- **Máquinas.** A prioridade continua: 16 linhas por tick no máximo, e a
  radiação cede nos ticks de transação.

**Testes:**
- **`id_samples_count_exactly_without_tags`:** cada item; o misto com vanilla;
  `MAX_RATE ± 1`; o mais quente ×1..40 e ×2^20; o catálogo todo; e pares dos 6
  itens mais quentes, cada um saturado, sem wrap.
- **`an_unknown_id_never_reads_another_items_count`.**
- **`unknown_tags_switch_to_id_samples_and_keep_the_dose`:**
  - um jogo sem as tags troca de modo;
  - a dose sai só de amostras confirmadas, em torno de 700 mRAD/s, com o HUD
    certo;
  - o jogo passa a conhecer as tags, a tentativa seguinte volta a `Tags` e o
    ritmo volta a 1/s;
  - reentrar no mundo começa em `Tags`.
- **`unconfirmed_samples_without_cheats_never_switch_modes`.**

**Mutações pegas:**

| Mutação | Teste que falha |
| --- | --- |
| nunca trocar | o da troca |
| dose de 1 s por amostra por ids | o da troca (o ritmo) |
| caminho compartilhado entre itens | o do id desconhecido |
| sem tentativa de voltar às tags | o da troca |
| `Incomplete` contado como tag desconhecida | os 3 de "sem comandos" |
| sem o limite por item | o de pares saturados |

## Verificação em jogo (2026-10-29)

Os jogos rodaram em pastas temporárias, mudos (options com master/music 0,0,
`HORIZON_NO_AUDIO=1`, `mute.py`), na tela virtual Xvfb `:12` e via `agent-run`.
A instalação real não foi tocada. Imagens em `img/radiacao-bits/`.

**Horizon com patch** (`/home/sacer/Workspace/horizon-tags` = upstream
9e4903787 + só `mod-item-tags.diff`, release), modo tags:

| Teste | Resultado |
| --- | --- |
| `clear @s #ntm:rad/bit_8 0` com 2 lingotes | "Found 2"; `bit_0` → "No items"; `rate_350` → "Found 2" (sem patch: "Unknown item tag") |
| 2 lingotes de urânio | 48 amostras de 0,700 aplicadas (dose 33,60 RAD exata); nunca trocou para ids |
| Misto: 3 lingotes + 2 pó de tório + 4 pepitas U-238 + 1 tarugo UZH + 7 células de trítio + 64 pedras | HUD **1,819 RAD/s**, exato (`patch-misto-1819.png`) |
| Ovo de balefire | HUD **16777,215 RAD/s** (o teto), morte em 1000 RAD e dose zerada, como no NTM (`patch-teto.png`) |
| Queimador ao lado, 3 cliques "Pôr combustível" | carvão 5 → 2 na mão, queimador ligado com 2 na fila (3 creditados), radiação seguiu em 0,700 (`patch-queimador.png`, `patch-depois-queimador.png`) |

**Horizon atual sem patch** (`/home/sacer/Games/horizon`), modo ids:

| Teste | Resultado |
| --- | --- |
| Troca | log: "the game doesn't know the mod's item tags in commands; sampling by item ids", logo após entrar |
| Misto (o mesmo acima) | HUD **1,819 RAD/s** + "Modo compatível: leitura por ids a cada 3 s" (`ids-misto-1819.png`) |
| Ritmo (build final, com dose por tempo decorrido) | 38,57 → 83,13 RAD em 25,2 s = 1,77 RAD/s (97% de 1,819; a diferença é a granularidade de uma amostra de ~3,5 s), uma só sessão (`final-ids-1819.png`) |
| Ovo de balefire | 16777,215 e morte, como no modo tags (`ids-teto.png`) |

**Não verificado em jogo:**
- tags de bloco (o conteúdo não tem nenhuma);
- a volta de ids para tags no meio de uma sessão (só no mundo falso);
- a máquina no modo ids.

Build final (com a correção de sessões e de dose do agente 2a5af3a4), Horizon
com patch: modo tags, misto em **1,819 RAD/s**, dose 41,84 → 87,31 RAD em
25,1 s = 1,81 RAD/s (99,5%), uma só sessão e nenhuma troca para ids
(`final-tags-1819.png`).

Sob carga alta (load ~22), sessões do cliente reiniciavam por saltos de
game-time. O agente 2a5af3a4 corrigiu isso (`fundacao.md`, "Resets de sessão
e fuel sob carga"); a rodada final por ids teve uma só sessão.

## Próximos passos

Radiação de chunk/área, blocos colocados, itens no chão e contaminação de
máquinas ficam para uma próxima etapa. Exigiriam estado espacial, alcance,
blindagem e cadência próprios; não se confunde o item de um minério no
inventário com o bloco colocado. Também ficam para depois a distribuição
aleatória de sintomas, tratamentos e proteção por equipamento.

## Verificação do M1 (concluída em 2026-10-28, desenho anterior por item)

`cargo test --workspace`: **63 testes passaram**, incluindo oito testes de
radiação (três do modelo/tabela e cinco de integração). `cargo clippy
--workspace --all-targets -- -D warnings`: limpo. A extração de hazards é
idempotente. Scripts Python compilam e os scripts shell passam em `bash -n`.

A distribuição verificada está em `target/rad-final-dist`; o jogo isolado foi
`target/test-game/radiation-final`, mundo `RadiationFinal`. O log confirma
`HORIZON_NO_AUDIO=1`; master e music ficaram em 0,0 no options temporário,
com `mute.py` ativo, e o cwd foi a pasta temporária. O jogo foi fechado ao fim.
Nenhuma instalação real de jogador foi usada; nenhum commit foi feito.

| Cenário no build final | Resultado observado |
| --- | --- |
| Um lingote | HUD 0,350 RAD/s, dose crescente, item conservado |
| Tecla Q | Inventário vazio, HUD 0,000 RAD/s, dose mantida |
| 64 lingotes | HUD 22,400 RAD/s; os 64 itens continuaram no inventário |
| Dose acima de 200 RAD | Náusea e fraqueza aplicadas |
| Dose 721,35 RAD | Náusea, fraqueza III, lentidão III e veneno II; saúde 20→14 |
| Ctrl+Q, sem recolher a pilha | Inventário vazio, taxa zero; dose ficou em 721,35 RAD |
| Fechar e reabrir, sem `/ntmlogic save` | HUD e save mantiveram exatamente 721,35 RAD; inventário vazio |

`tools/test-game/radiation.py` executou os dois estágios com assertions.
A evidência JSON e as capturas `single.png`, `dropped-single.png`,
`sickness-200.png`, `sickness-600.png`, `dropped-stack.png`, `saved.png` e
`reentered.png` estão na pasta do jogo. O save foi `w.1460677603`, com
721350 milliRAD para o UUID do jogador. Os checksums SHA-256 dos pacotes
usados estão no mesmo `radiation-evidence.json`.

O ensaio preliminar em `target/test-game/radiation-m1` também verificou um
lingote somente na mão secundária (0,350 RAD/s) e as faixas de 800 RAD,
incluindo wither/dano e morte. Os testes do modelo cobrem dano/zeragem em
1000 RAD. **Não foram executados crash real do processo nem o limiar letal
de 1000 RAD no jogo final.** Crash no meio da coluna radioativa, restauração
sem dose antiga, timeout, save automático, limite de frequência de saves e
save imediato ao cruzar limiar foram verificados no mundo falso.

O helper `ctl drop` altera a janela sem enviar o drop ao servidor integrado
nessa versão do Horizon (`game/agent.rs:1448`); o teste usa a tecla Q real
(`game/remote.rs:5218`). Esta é uma limitação do helper, não da consulta de
radiação. O driver aguarda o handshake inicial do servidor antes de dar itens.

Para reproduzir em uma distribuição **nova**:

```sh
CARGO_TARGET_DIR="$PWD/target/rad" DIST="$PWD/target/rad-replay-dist" tools/build-logic.sh
DIST="$PWD/target/rad-replay-dist" tools/test-game/run.sh radiation-replay
```

Em outro terminal, após o socket abrir:

```sh
tools/test-game/ctl.sh radiation-replay open RadiationReplay 345680 creative
python3 tools/test-game/radiation.py radiation-replay target/rad-replay-dist
tools/test-game/ctl.sh radiation-replay quit
```

Reabra com o mesmo `run.sh`, abra `RadiationReplay` e rode:

```sh
python3 tools/test-game/radiation.py radiation-replay target/rad-replay-dist --reenter
```
