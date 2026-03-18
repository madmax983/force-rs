import json

with open('mutants.out/outcomes.json', 'r') as f:
    data = json.load(f)

for o in data.get('outcomes', []):
    if o.get('summary') == 'Missed':
        print(o.get('scenario', {}).get('Mutant', {}).get('replacement'))
