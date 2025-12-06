#!/usr/bin/env python3

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

test_data = [64, 34, 25, 12, 22, 11, 90, 88, 45, 50, 33, 17, 19, 82, 72, 95, 3, 7, 29, 41]
result = quicksort(test_data)
print(f"Sorted: {result}")
