def fib(n):
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)

def main():
    n = 40
    ans = fib(n)
    print(f'fib({n}) = {ans}')

if __name__ == '__main__':
    main()
