#!/usr/bin/env python3
import random
random.seed(42)
data = [random.randint(1, 1000) for _ in range(1000)]
print(str(data).replace(' ', ''))
