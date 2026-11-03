# Catálogo em massa K2: consumíveis, kits e materiais soltos

Entram 116 itens, levando o catálogo a 3.191 (593 blocos e 2.598 itens). Na importação K2, todos entraram como itens inertes: identidade, textura, nome (en_US e pt_BR) e os hooks do fonte listados como lacuna. A fundação de hooks ativa um lote representativo, descrito abaixo.

## Reconhecimento genérico de classes de item (bulk.py)

Além da regra de cubo do K1, o importador agora mapeia classes de item para uma forma, sem casos isolados (`static_item`):

- **Item inerte de uso**: uma classe cuja ancestralidade não toca nenhuma base de equipamento ou estado de máquina e que não sobrescreve nenhum hook de renderização por metadata (`registerIcons`, `getIconFromDamage`, `getColorFromItemStack`, `getSubItems`, passes de render) entra como item. Os hooks de uso que ela tem (`onItemRightClick`, `onEaten`, `onItemUse`, `hitEntity`, `getMaxItemUseDuration`, `getItemUseAction`, `addInformation`, durabilidade...) viram `behavior_gap` explícito, e o NBT por pilha também fica anotado. `setMaxDamage` limita a pilha a 1 (sem `max_damage`, como nos demais casos).
- **Bases proibidas**: `ItemArmor`, `ItemTool`, `ItemSword`, `ItemHoe`, `ItemBucket`, `ItemRecord`, `ItemArmorMod`, `ItemFuelRod`, `ItemNuclearWaste`, `ItemEnumMulti`, `ItemGenericPart`, `ItemBattery`, `ItemKeyPin`, `ItemCraftingDegradation` continuam auditadas.
- **Outras famílias** continuam fora, por classe ou nome: mísseis e partes (`mp_*`, `missile_*`), armas e munição, satélites e chips, designadores, detonadores e granadas, pilha e placas legadas, resíduos, armaduras e modificadores, instrumentos (Geiger, dosímetro, detectores, scanners, linkers, RTTY, sensor do reator), varinhas, sifão e itens de lote próprio.
- Linhas já em `bulk.json` ficam congeladas e nomes já declarados em outros catálogos não são reclamados. A regeneração acrescentou 116 itens e não removeu nem alterou nenhum.

## O que entrou

| Classe do fonte | Itens |
| --- | ---: |
| `ItemEnergy` (latas e garrafas de bebida) | 19 |
| `ItemStarterKit` (kits de bombas, armaduras e custom) | 19 |
| `ItemLemon` e comida (limão, queijo, chocolate, comidas especiais, remédios) | 17 |
| `ItemDrop` (células, singularidades, pellets) | 10 |
| `ItemPill` (pílulas) | 9 |
| `ItemSyringe` (seringas) | 6 |
| `ItemFilter` (filtros de máscara) | 5 |
| `ItemSoup`, `ItemLootCrate`, `ItemCigarette` | 8 |
| Materiais e utilitários soltos (cada um com classe própria: `rag`, `plastic_bag`, `casing_bag`, `bdcl`, `matchstick`, `nugget`, `powder_fertilizer`, `particle_digamma`, `crystal_xen`, `loops`...) | 23 |

Lista completa em `content/ids.json` (as linhas novas deste commit). Os itens de bomba e de míssil de `ItemStarterKit` aparecem nas abas do criativo de bombas e mísseis, como no fonte.

## Lacunas

- Nada tem efeito: comer, beber, injetar, abrir caixas de loot, fumar, kits que entregam itens, filtros que gastam, células e singularidades com NBT, tudo isso é lógica nativa de uso. Pílulas e comida não são comestíveis (o template é o item inerte padrão; nenhum valor de fome é copiado).
- Durabilidade e estado por pilha (`ItemFilter`, `ItemRepairKit`, kits) ficam em NBT nativo.
- Fora do K2, pelas regras acima: `ItemHot` (lingotes quentes que dependem de NBT de temperatura), `ItemInfiniteFluid`, `ItemPipette`, as seringas `syringe_antidote`/`awesome`/`poison` e `iv_*` (declarações que o scanner ainda não resolve), `radaway*`, e os instrumentos listados.

## Efeito nas tabelas

Receitas que dependiam desses itens e estavam em `skipped` passaram a resolver: bigorna +1 (143), planta química +2 (62), cristalizador +1 (405 emitidas; `pill_red` deixou de ser uma lacuna), e a receita da barra RBMK `rbmk_fuel_drx` (balefire + `particle_digamma`), já documentada no R2. `crates/ntm-core/data/furnace_recipes.json` ganhou três receitas de forno do fonte (`glyphid_meat` → `glyphid_meat_grilled`, `rag_damp` e `rag_piss` → `rag`); o gerador as trata como receita de forno nativa e não emite o equivalente vanilla (a regeneração só estabiliza assim, verificada em duas passadas). Hazards: +116 identidades, com uma radioativa (`demon_core_open`, 5 RAD/s); nenhuma taxa antiga muda. Buckets regenerados.

## Prova e validação

`verify-catalog-batch.py` entrega os 116 itens pelo inventário no binário nativo, mudo, com detector de rejeição: 116 IDs, 144 conferências, sem `reject` nem `panic` no log do cliente. 208 testes por seed 1/42 (inclui as regras de classe de item em `test_bulk_forms.py`), `validate-bulk` e `recipe-gen/validate` verdes. Pacote `ntm.hmod` SHA-256 `353fa287a9cd1b20ae6eceed2775a7aafe94bf47f8d20389ab6c390f09e0ff6c` (sobre o K1), aprovado no check do upstream e do fork.


## Lote representativo de hooks (em validação)

`cotton_candy`, `syringe_metal_medx`, `hazmat_kit` e `gas_mask_filter` usam
agora a API nativa de uso/estado. O lote acrescenta `radaway`, `iv_empty`,
M65, quatro peças hazmat amarelas e `attachment_mask`, com a mesa de
modificação. O uso de coleta de sangue de `iv_empty` continua fora do lote.
Os demais consumíveis K2 não foram ativados. Contratos e evidências atuais
estão em [Hooks nativos de item](native-item-hooks.md) e no artifact `item-hooks`;
as provas históricas acima dizem respeito à importação do catálogo.
