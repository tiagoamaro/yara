def classify(n: Int): String
  if n > 0
    "positive"
  elsif n < 0
    "negative"
  else
    "zero"
  end
end

print(classify(5))
print(classify(-3))
print(classify(0))
