# Conteúdo e receitas do triturador

`content/machine-recipes/shredder.json` (schema 1) contém **722 receitas**, das quais 463 vêm dos defaults e 259 do pós-processamento OreDict. As 186 chamadas explícitas incluem expansões finitas de madeira, cores, minérios de bedrock e metadata wildcard. O índice de referência tem 620 chaves; há 196 omissões explicadas (54 defaults e 142 filtros do pós-processamento) e 54 colisões registradas.

O JSON é compacto por ser uma tabela gerada grande. Cada receita conserva `name`, `input:{ids,count}`, `outputs:[[id,count]]`, `label`/`label_en`, fase e referência fonte. As receitas geradas também incluem a chave OreDict e sua proveniência. `skipped` registra o motivo; `collisions` registra a receita conservada e a tentativa ignorada. Não há consultas de rede na geração.

## Precedência do original

`ShredderRecipes.setRecipe(ComparableStack, ItemStack)`, linhas 417–420, só insere quando a chave ainda não existe. Portanto **registerPost não sobrescreve registerDefaults**. Por exemplo:

| Entrada | Saída conservada | Tentativa automática ignorada |
|---|---|---|
| Minério de diamante vanilla | 2 cascalhos de diamante | 2 pós de diamante |
| Bloco de quartzo vanilla | 4 pós de quartzo | 9 pós de quartzo |
| Minério de alumínio NTM | 2 fragmentos de criolita | 2 pós de alumínio |

A busca fonte consulta metadata exato antes do fallback wildcard; o exportador expande o wildcard somente para os IDs já catalogados. Entradas sem receita conservam `fallback_output:["ntm:scrap",1]`, conforme `getShredderResult`.

O pós-processamento aplica os prefixos e multiplicadores do Java: lingote/placa/gema/cristal ×1, minério ×2, bloco ×9 (ou ×4 sem lingote/gema), dustTiny → `ntm:dust_tiny`, dust → `ntm:dust`. Chaves contendo `Any` são excluídas. Saídas de pó ausentes ou equivalentes a scrap não produzem receita automática. O perfil usa a preferência fonte padrão `[hbm]`, traduzida para `[ntm]`; mantém o primeiro pó NTM registrado.

As equivalências vanilla seguem o [OreDictionary do Forge 1.7.10](https://github.com/MinecraftForge/MinecraftForge/blob/1.7.10/src/main/java/net/minecraftforge/oredict/OreDictionary.java#L53-L105), verificadas contra o registro 26.3. Variantes modernas sem equivalente legado (como deepslate) não são inventadas. Configuração de outros mods, IMC, bobbleheads dinâmicos e identidades ausentes do catálogo ficam fora, com lacunas explícitas quando uma chamada fonte é omitida.

## Bloco e lâminas

`ntm:machine_shredder` usa `minecraft:beehive`, `facing` e `honey_level=0..5`, com 24 variantes, loot próprio e nomes fonte `Shredder`/`Triturador`. Consta em `ids.json.machines`. O fallback cúbico usa as quatro texturas NTM `machine_shredder_{side,top,bottom,front}_alt`, compostas sobre aço. A sobreposição de modelo/tela pertence a `tools/visual-gen`.

Arquivos principais:

- `content/data/ntm/horizon/block/machine_shredder.json`;
- `content/assets/ntm/blockstates/machine_shredder.json`;
- `content/assets/ntm/models/block/machine_shredder_{0..5}.json`;
- `content/assets/ntm/items/machine_shredder.json`;
- `content/data/ntm/loot_table/blocks/machine_shredder.json`.

A montadora passa a ter a receita real `ass.shredder`: oito placas de aço, quatro de cobre e dois motores; 100 ticks e 100 HE/t. O arquivo `assembly.json` passa de 398 para 399 receitas. Nenhuma ponte `m1_temp__` foi retirada.

As três lâminas já existiam e empilham até um. A seção `blades` do JSON registra aço=400, titânio=500 e desh=0/sem desgaste, extraídos dos construtores `ItemBlades`. O gerador também registra esses dados no catálogo.

`--max-damage` é uma extensão **opt-in** para o motor com o patch `ntm-visual`. Ativada, emite `max_damage:400`/`500` nas declarações das lâminas de aço/titânio. Desh sempre omite o campo. O padrão mantém a compatibilidade com API 0.2 base, que rejeita campos desconhecidos; nesse modo o desgaste pode ser administrado pela lógica, mas a barra de durabilidade não aparece. O template continua sendo um item comum, sem herdar comportamento de arco ou ferramenta vanilla.

## Reprodução e validação

```sh
/usr/local/bin/agent-run python3 tools/content-gen/shredder_recipes.py
/usr/local/bin/agent-run python3 tools/content-gen/test_shredder.py
# Pipeline completo; restaura a sobreposição visual autorizada em seguida:
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/visual-gen/generate.py
# Somente se o alvo tiver a extensão max_damage:
durable_content=$(mktemp -d /tmp/ntm-durable-content-XXXXXX)
cp -a content/. "$durable_content/"
/usr/local/bin/agent-run python3 tools/content-gen/generate.py --max-damage --output "$durable_content"
/usr/local/bin/agent-run python3 tools/visual-gen/generate.py --out "$durable_content"
```

Sete testes do triturador verificam precedência real, multiplicadores, laços/metadata, todos os IDs de entrada/saída, lacunas, determinismo e a flag de durabilidade em saídas temporárias. Os sete testes M3 existentes e a validação integral dos **2516 IDs** também passaram. Build/check do pacote base passou, assim como `cargo test --workspace -j 3` (179 testes, dois ignorados). O agente de lógica é dono do conversor `tools/machine-gen/build.py`, dos dados Rust e da verificação funcional em jogo. Logs desta frente ficam em `content-gen-evidence/shredder-validation.log` e `shredder-build.log`.
