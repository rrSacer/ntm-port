# Energia, lote 4: caldeira solar, espelho e Stirling

## Integração nativa retomada em 02/11/2026

As cinco máquinas agora têm simulação nativa de calor, vapor e HE, com vínculo pela ferramenta `ntm:mirror_tool`. A variante creative continua exigindo calor. O estado de engrenagem, excesso de velocidade e alvos é persistido nos block entities. O K3 e esta frente foram rebaseados no Horizon oficial `d928d1cd6`, pin integrado `3eb781ab5`, e passaram na aceitação nativa. A falha da engrenagem é simulada; o aviso sonoro, a explosão de overspeed (inclusive seu dano a entidades) e o projétil de engrenagem ainda não são emitidos.

**Defeito nominal (perda em relação ao Java):** na falha, o port faz `has_cog = false` e a engrenagem some. No Java ela vira o `EntityCog`, e `EntityCog.interactFirst` devolve um `gear_large` com o metadado da variante ao jogador que a pega. A explosão de força 5, sem fogo e sem quebra, entra pelas decisões D1+D2 do artifact `onda2-efeitos-mundo` (o Stirling é o piloto). A devolução da engrenagem depende da API de entidades e fica para a Onda 4 do `roadmap-100`. Não é corrigida agora.

### Verificação da integração atual

No servidor integrado, com combustível, rede, água e ferramenta reais, `tools/test-game/native_solar_stirling.py` passou nas quatro fases, com 15 asserções:

| Prova | Resultado |
| --- | --- |
| Stirling ferro e aço | Bateria passou de 565 para 21.136 HE em cada cenário; engrenagens intactas. |
| Stirling creative | Bateria passou de 1.135 para 42.295 HE. |
| Sem fonte quente | As três baterias pararam de crescer depois da retirada do firebox. |
| Solar | 13 espelhos vinculados pela ferramenta; vapor passou de 7.400 para 16.000 mB de dia. |
| Noite e teto | Vapor ficou em 16.000 mB à noite, espelho desligado; teto opaco também o desligou ao meio-dia. |
| Reabertura real | Após `capture-state`, encerramento limpo e novo processo, `check-state` conferiu igualdade das 21 entidades: três motores, três baterias, caldeira, barril de vapor e 13 espelhos. |

`ntm-core`: 269 testes unitários e 26 de integração passaram. Os 213 testes Python e os validadores de conteúdo/receitas passaram no lote integrado. No motor, passaram 121 testes `native_` e 29 testes de menus, além de Clippy com avisos tratados como erro para as bibliotecas afetadas e o `ntm-core`. O build nativo release passou.

Evidência local: `.agent-tmp/energy-b_9zzycu/native-solar-stirling*.json`, relativa à raiz do projeto. O pacote integrado tem 9.093.273 bytes, SHA-256 `06b31d447623a7c88a2cc440f6866cd570d5d2d09b2d3a1cbb12395a0757cac8`. O cliente e o Xvfb de teste foram encerrados; não houve instalação nem abertura de mundos do jogador.

Limites: a prova foi no Overworld, com tempo limpo e geometria controlada. A amostragem solar lê o mundo ativo ou estacionado da dimensão pedida, e o acesso aos block entities (`Host::entity`) também chega às dimensões que não ticaram por último (`docs/host-dimensoes.md`); a prova em jogo dessa correção usa baterias, não os geradores solares/Stirling, e a reabertura de um mundo com geradores em outra dimensão não foi exercitada aqui. Clima, falha/reparo em jogo e reabertura com overspeed não zero não foram cobertos por esta prova. Os efeitos de overspeed ausentes são os nomeados acima.

O trecho abaixo conserva o histórico e o contrato do conteúdo anterior.

Entram **cinco máquinas**: `machine_solar_boiler`, `solar_mirror`, `machine_stirling`, `machine_stirling_steel` e `machine_stirling_creative`. O catálogo chega a **2.700 IDs** (427 blocos, 2.273 itens), **76 máquinas** e **936 receitas vanilla**. Os blocos têm nomes originais en_us/pt_br, loot próprio e o barramento beehive com facing e honey_level 0..5.

## Contrato inicial de conteúdo

O lote inicial entregou conteúdo e o contrato algorítmico `content/machine-recipes/solar_stirling.json`. As cinco máquinas ficam registradas como **Inert** (`bc6b2727`), sem lógica nova de energia nesta fase da migração. Os cubos são fallback com texturas do NTM; `auto: false` impede importar modelos OBJ automaticamente. Multiblocos, portas, animações, luz solar e ferramenta de apontamento ainda dependem do runtime/visual. Existir no catálogo não significa funcionar como gerador.

## Construções originais

- **Caldeira solar:** bancada, `SHS / DHD / SHS`: lingotes de aço, cascas de aço e corante preto (`KEY_BLACK`).
- **Espelho:** bancada, `AAA / ␠B␠ / SSS`: placas de alumínio, viga de aço e lingotes de aço; produz **três espelhos**.
- **Stirling base:** bigorna tier 2, 16 tábuas, seis placas de aço, oito lingotes de cobre, quatro bobinas de cobre e uma engrenagem grande de ferro.
- **Stirling steel:** bigorna tier 2, 16 placas de aço, seis berílio, quatro cobre, oito bobinas de ouro e uma engrenagem grande de aço.
- **Creative:** sem receita de sobrevivência no NTM; nenhuma foi inventada.

As engrenagens já existiam desde o lote 2. O resolver da bancada não registrava `shell` entre as formas de material, embora `shell__steel` já existisse e o resolver de máquinas já o suportasse. Acrescentar a forma ao mapa existente destravou a caldeira e, como efeito adicional, a receita original de `assembly_nuke` com `shell__weaponsteel`. São **três receitas vanilla novas**, sem alternativas temporárias; bulk 649→652. O OreDict continua usando o snapshot conservador existente.

A bigorna passa de **114/143 para 118/139** (emitidas/skipped): duas construções e duas reciclagens dos Stirling completos. As variantes de item sem engrenagem (metadata 1) continuam como lacuna, sem criar novos IDs improvisados. As alternativas de descarte/reciclagem que as exigem permanecem em skipped.

## Contrato do fonte

Schema 1, cinco `machine_config`, `recipes: []`, `skipped: []`. São algoritmos por tick, sem potência ou duração de crafter fabricadas.

| Máquina | Regra preservada |
|---|---|
| Caldeira solar | Tanques de 100 mB de água e 10.000 mB de vapor. `process=heat/50` com divisão inteira, depois limitado pela água e por `freeSteam/100`, nunca negativo. Cada água produz 100 vapor. `display` recebe o valor antes dos limites. O calor inteiro é zerado todo tick, inclusive sobra; não existe armazenamento inventado. |
| Espelho | `sun=savedSkyLight-skylightSubtracted-11`. Só contribui se positivo, vendo o céu acima e com targetY≥mirrorY. Soma `sun` à caldeira em `(targetX,targetY-1,targetZ)`. Pode ficar `isOn=true` sem uma caldeira válida. O acumulador e sua limpeza seguem a ordem dos ticks das máquinas. |
| Stirling base/steel | Puxa `trunc(sourceHeat*0,1)` de um IHeatSource imediatamente abaixo. Gera `trunc(heat*0,5)` HE no buffer do tick. O limite de overspeed é 300 TU no base e 1.500 no steel; não é um teto que descarte calor. Mais de 300 ticks consecutivos acima do limite perde a engrenagem, gera explosão de força 5 sem dano a blocos/fogo e ejeta o projétil. |
| Stirling creative | **Ainda precisa de calor.** Mesma difusão 0,1, eficiência 1 e sem overspeed. Não é uma bateria infinita. O valor 1.500 de maxHeat participa da animação, sem limitar a geração. |

O Stirling fornece o buffer anterior antes de zerar/recalcular o novo. Sem engrenagem, a velocidade de fornecimento é zero e o buffer residual positivo cai uma unidade por tick. O calor é zerado ao final do tick. As engrenagens de reparo são ferro no base e aço no steel; a variante creative usa metadata 2 no fonte, sem item correspondente declarado.

**Detalhe do fonte conservado:** `maxHeatNormal` e `maxHeatSteel` são campos configuráveis, mas `maxHeat()` usa os literais 300/1500. O contrato registra os valores efetivos; não aplica configurações externas.

A ferramenta original grava o core da caldeira com Y+1. Ao vincular um espelho, limita a distância a **100 blocos inclusive** e exige `dx²+dz²≤dy²`. Esses dois limites são da ferramenta, não uma revalidação por tick no espelho. Esse vínculo agora é implementado pelo bundle nativo.

## Verificação histórica de conteúdo

- **168 testes Python por seed 1 e 42**, incluindo seis testes do lote: IDs/estados/loot, construções originais, ausência de receita creative, limites solares, vínculo da ferramenta e semântica dos Stirling. Os validadores de conteúdo e receitas passaram.
- Build/check: **6.356.238 bytes**, SHA-256 `ce49762047b1d29895f8f7bca5f4af0a1819511ea4849bdb264baad8230a94f0`. Durabilidade segue `components`, já aceito no fork após C1.
- Prova **horizon-patched**, content-only, Xvfb e áudio desligado, mundo temporário: **66 conferências**, `/setblock` dos cinco IDs nos níveis 0 e 5 e `/give` dos cinco itens, confirmados no inventário. Detector de rejeição/panic vazio. Isso prova carregamento e identidade dos blocos/itens, não geração funcional de calor ou HE.
- Regeneração limpa de `fa0862a1` com content-gen e visual-gen: **14.024 arquivos comparados sem diferenças**, incluindo conteúdo, hazards, buckets e receitas de fornalha. O agente de lógica registrou os cinco Inert (`bc6b2727`) e regenerou suas tabelas (`a7d1aaeb`), incluindo a bigorna com 118 construções. Na exportação isolada de `a7d1aaeb`, `cargo test --workspace` passou (**296 testes do ntm-core**) e `cargo clippy --workspace -- -D warnings` ficou limpo. Esses commits não alteraram o conteúdo empacotado/provado.

Evidências em `content-gen-evidence/energy-solar-stirling/`, com os logs Python em `content-gen-evidence/energy-solar-stirling-tests-{1,42}.log`. Nenhuma instalação ou mundo do usuário foi alterado.

Solução mais simples verificada: reaproveitados `energy_catalog`, o mapa de formas do resolver, o formato algorítmico dos lotes anteriores, o pipeline de assets e o perfil de prova existente. A integração nativa da migração não exige trocar o formato do content mod; não foi criado um formato de runtime alternativo.
