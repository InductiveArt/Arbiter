#pragma once

namespace geo {

enum class Shape { kCircle, kSquare };

class Circle {
 public:
  double radius() const;
  double diameter() const;
};

double area(const Circle& circle);
double perimeter(const Circle& circle);

}  // namespace geo
