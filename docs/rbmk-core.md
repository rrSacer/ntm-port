# RBMK: núcleo puro de simulação (frente T1)

Módulo `crates/ntm-core/src/rbmk/`, tickets `rbmk-plano` T03, T04, T07, T08 e T09 (T01/T02 e o que depende do motor ficam para depois). Tudo aqui é aritmética e dados, sem o motor: o módulo recebe o estado das colunas e devolve o novo estado mais uma lista de **efeitos** (radiação, pedidos de meltdown, respiros de vapor) que o servidor aplica.

## O que entrou

| Módulo | Porta | Prova |
|---|---|---|
| `dials` | `RBMKDials` (25 dials: chave, padrão, parse Java, clamps, cache de `createDials`/`refresh`, `getSurgeMod` do cliente) | 25 dials × 10 entradas, bits |
| `rods` + `rods_data` | `ItemRBMKRod`: 33 barras, 9 curvas de queima, 5 de depleção, xenônio, `burn`/`updateHeat`/`provideHeat`, tag da pilha (`yield`/`xenon`/`core`/`hull`) | 360 + 35 pontos de função, 33 barras × 4 ticks, 14 casos discriminantes |
| `jmath` | `Math.log10`, `pow`, `sin`, `cos` do HotSpot (ver abaixo) | 7500 amostras do JDK |
| `jrandom` | `java.util.Random` serializável (D3) | usado pelos testes de fluxo ReaSim e do respiro |
| `reactor` | passo do servidor: fila de fluxos do tick anterior → atualização das colunas em ordem de registro (D1). Barra de combustível, barra ReaSim (8 fluxos angulares), moderador, refletor, absorvedor, em branco, controle manual/automático/energizado, **outgasser**, **boiler**, **heater**, **cooler**, calor entre vizinhos, caldeira ReaSim | 38 cenários, bit a bit por tick |
| `outgasser`, `fluids`, `tables_data` | 19 receitas do outgasser (fixture executada), tanques, `FT_Heatable` (15 fluidos) | idem |

Provas (cenários em `tools/rbmk-oracle/flux_scenarios.json`): linha de barras, núcleo 5×5, refletor/absorvedor com eficiência 1 e 0,5, controle manual com surto, controle automático (três funções e faixa degenerada), controles energizados, barras ReaSim (ângulos, alcance 7), obstáculos parciais/totais e coluna alta, fluxo de calor com e sem caldeira ReaSim, superaquecimento com meltdown ligado e desligado, inserção/remoção de barra, dials de depleção/xenônio/reatividade, as 33 barras, boilers de cada grau de vapor, respiro com RNG, compressor, heater (5 fluidos e identificador), cooler (janela 5×5, atualização a cada 60 ticks), outgasser (sólido, fluido, ambos, bloqueios), fronteiras `progress > 10000` (9949/9950/9950,5) e `heat > 1500` (1 ulp abaixo, igual, 1 ulp acima, +0,5), refletor/absorvedor com eficiência 0,3 e 0,8, anel ReaSim de longo alcance (distingue 9° de 9,5°) e um heater para cada um dos 15 fluidos com `FT_Heatable` (a classe Java real roda todos). Mutações do revisor mortas por esses testes: `*= 1-eff` (absorvedor e refletor), `>=` no progresso e no calor, `MAX_HEAT` 1501, passo ReaSim 9,5°, `sin` da depleção pela libm. **Sobrevive:** `pow(1-level,15)` pela libm (a libm acerta os mesmos 3000 pontos; ver resíduo acima).

## Como a prova funciona

`tools/rbmk-oracle/run_flux.py` compila **as classes Java inteiras e inalteradas** (`TileEntityRBMKBase/Rod/RodReaSim/Control*/Absorber/Moderator/Reflector/Blank/Outgasser/Boiler/Heater/Cooler`, `RBMKNeutronHandler`, `NeutronStream/Node/NodeWorld/NeutronHandler`, `FT_Heatable`, `BlockPos`, `Vec3NT`) contra um mundo de blocos simulado (`stubs_flux.py`: só identidade, contêineres e apresentação) e roda os cenários tick a tick, gravando todo o estado em `fixtures/flux.json`. Os testes Rust (`tests/rbmk_oracle_parity.rs`, `tests/rbmk_flux_parity.rs`) reconstroem o mesmo cenário a partir do JSON e comparam cada `double` pelos bits após **cada** tick. Regenerar: `python3 tools/rbmk-oracle/run_flux.py --scratch <dir fora do repo>` (o `--check` compara).

Dados gerados: `gen_rods_rs.py` (`rods_data.rs`, das 33 barras da fixture), `gen_rbmk_tables.py` (`tables_data.rs`: 19 receitas do outgasser da fixture e as linhas `FT_Heatable` lidas do `Fluids.java`), `gen_flux_scenarios.py`.

## Desvios e limites (todos nominais)

- **`Math` do HotSpot** (`jmath`): a libm da plataforma difere do JDK no último bit (`log10` em 21 % dos argumentos). `jmath` avalia em precisão dupla-dupla e arredonda uma vez: `log10` bate em 4500/4500, `pow(x,15)` e `pow(x,2)` em 3000/3000; `pow(E,y)` erra 2/1500, `sin` 5/3000, `cos` 6/1500 e o `sin` dos argumentos da depleção 5/4500 (os stubs Intel do HotSpot não são corretamente arredondados e o algoritmo não está disponível). Resíduo de 1 ulp no último bit, fixado por função em `jmath_matches_hotspot`. Nos pontos de chamada reais (varredura de 9000 enriquecimentos e 3000 surtos, `depletion_and_surge_sweeps_match_java_bits`): depleção 6/9000 e surto 1/3000 diferem 1 ulp. **A referência é o intrínseco do JDK 25 em x86-64;** o Minecraft 1.7.10 original roda em Java 8, que usa outro caminho para `Math.sin/pow`, então a paridade bit a bit vale para o JDK do oráculo. `sin`/`cos` com |x| > 1e5 usam a libm.
- **Cache de nós do Java** (`NeutronNodeWorld`): o port consulta as colunas ao vivo. A única diferença visível seria uma entrada de cache obsoleta de uma coluna removida há menos de 20 ticks (ou a tampa lida do cache). Decisão D1 (ordem lógica, sem história de HashSet).
- **Bugs do original preservados (D2):** radiação do fluxo parcialmente bloqueado cai na célula de origem, refletor com eficiência < 1 deixa o fluxo seguir, `getFluxRange` também no ReaSim, o teto de 10 000 °C do `TileEntityRBMKRod` é código morto (o `return` do limite 1500 vem antes), controle automático com limites iguais gera `NaN` no alvo (o nível não se mexe). Cada caso está num cenário.
- **Pins de ordem:** o respiro do boiler consome `nextInt(2)`, `nextInt(2)`, `nextInt(10)`, `nextFloat()` nessa ordem; a rotação ReaSim consome `nextInt(4)` antes dos oito fluxos.
- **Remoção de coluna:** `Reactor::remove` mantém a ordem, descarta fluxos pendentes e devolve a coluna quando a barra tinha casco ≥ 1500 e meltdowns ligados (o `invalidate()` do Java); o motor roda o meltdown. O aviso de satélite (`SatelliteRayScan`, a cada 200 ticks com fluxo) sai como `Effect::SatelliteScan`.
- **Armazenamento:** `Kind::Storage` só troca calor. O limite de pilha da saída do outgasser vem de `Reactor::max_stack` (64 se ausente). `f64::min/max/clamp` do Rust tratam `NaN`/`-0.0` diferente de `Math.*` (só entrada corrompida). Cada `Reactor` tem a sua RNG (o Java usa o `world.rand` único; D3 aceita; o motor decide se compartilha entre camadas).
- **Meltdown:** o passo só avisa (`Reactor::meltdowns`) e zera o fluxo da barra, como o Java. Blocos, corium, detritos, Spear, sobrepressão e o conjunto flood-fill (ordem de HashSet) são do motor (T20/D9).
- **Barras de curva especial** (`specialFluxCurve`): nenhuma das 33 usa; não modeladas.
- **Heater:** a tabela `FT_Heatable` vem do `Fluids.java` por regex (verificada por `gen_rbmk_tables.py --check`); o oráculo roda a classe Java real contra os quatro fluidos WATER/OIL/COOLANT/PERFLUOROMETHYL com a declaração verbatim, mais AIR (sem eficiência) e LAVA (sem trait). Fluidos carregados de JSON de configuração não existem aqui.
- **Energia dos controles ReaSim:** `Reactor::transfer_power` implementa o `transferPower` padrão; a rede de energia é do servidor. `receivePower` do Java não é chamado aqui.
- **Entrada inválida (D2):** `Dials` trata parse/clamp como o Java; `NaN`/índices só são sanitizados na fronteira do save (ainda não ligada).

## O que falta ligar (depois do merge da Onda 0)

0. Quebra de bloco: `Reactor::remove`; o `rbmk_storage` entra como `Kind::Storage`; `Inlet`/`Outlet` não participam do calor.
1. Registrar os 33 blocos `Inert` `rbmk_*` como `Kind` (e um id de barra → `RodType::by_variable`).
2. Chamar `Reactor::step` uma vez por tick de servidor e dimensão; ler `rad`, `meltdowns`, `effects`; `obstacles` vem do mundo (blocos opacos acima da camada).
3. Persistir colunas, a tag das barras (`RodStack`), a RNG (`JavaRandom::state`) e a fila de fluxos (tem de entrar no ledger transacional do save).
4. Tanques do boiler/heater/cooler/outgasser ligados ao `FluidStore`, portas e loaders (posições: `getPorts*Loader`), `identifiers` do heater a partir do item do slot 0, `ore_names` da OreDict.
5. Energia dos controles ReaSim (`transfer_power`), hazards/tooltips dinâmicos das barras (T03), GUIs e console (T05/T06).
6. Detritos e meltdown (D9) e o som `leverStop` (D8).
