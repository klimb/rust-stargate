#!/usr/bin/env python3
import random

def quicksort(arr):
    size = len(arr)
    if size <= 1:
        return arr
    
    pivot = arr[size // 2]
    less = []
    equal = []
    greater = []
    
    for val in arr:
        if val < pivot:
            less.append(val)
        elif val > pivot:
            greater.append(val)
        else:
            equal.append(val)
    
    sorted_less = quicksort(less)
    sorted_greater = quicksort(greater)
    
    result = sorted_less
    for item in equal:
        result.append(item)
    for item in sorted_greater:
        result.append(item)
    
    return result

random.seed(42)
test_data = [random.randint(1, 1000) for _ in range(1000)]
result = quicksort(test_data)
print(f"Sorted {len(result)} elements")
