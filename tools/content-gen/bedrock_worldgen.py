"""S6 datapack bridge for flat bedrock deposits; five-cell disk approximation."""
PROFILES = [('overworld', -64, 773641), ('nether', 0, 773659)]


def generate(emit):
    for dimension, y, salt in PROFILES:
        name = 'bedrock_' + dimension; feature = 'ntm:' + name
        def put(kind, body): emit(f'data/ntm/worldgen/{kind}/{name}.json', body)
        put('structure_set', {'structures': [{'structure': feature, 'weight': 1}],
                              'placement': {'type': 'minecraft:random_spread', 'spacing': 1, 'separation': 0, 'salt': salt, 'spread_type': 'linear'}})
        put('structure', {'type': 'minecraft:jigsaw', 'biomes': '#minecraft:is_' + dimension, 'step': 'underground_ores',
                          'terrain_adaptation': 'none', 'start_pool': feature, 'size': 1, 'start_height': {'absolute': y},
                          'use_expansion_hack': False, 'max_distance_from_center': 16})
        put('template_pool', {'fallback': 'minecraft:empty', 'elements': [{'weight': 1, 'element': {'element_type': 'minecraft:feature_pool_element', 'projection': 'rigid', 'feature': feature}}]})
        put('placed_feature', {'feature': feature, 'placement': [{'type': 'minecraft:rarity_filter', 'chance': 10},
                               {'type': 'minecraft:in_square'}, {'type': 'minecraft:offset', 'x': 8, 'y': 0, 'z': 8},
                               {'type': 'minecraft:height_range', 'height': {'type': 'minecraft:constant', 'value': {'absolute': y}}}]})
        # A radius-one disk is exactly five cells, at exactly the bedrock floor.
        # Java uses center + eight independent Bernoulli(.5) neighbors (mean five).
        put('configured_feature', {'type': 'minecraft:disk', 'radius': 1, 'half_height': 0,
                                   'target': {'type': 'minecraft:matching_blocks', 'blocks': ['minecraft:bedrock']},
                                   'state_provider': {'type': 'minecraft:simple', 'state': {'id': 'ntm:ore_bedrock'}}})
