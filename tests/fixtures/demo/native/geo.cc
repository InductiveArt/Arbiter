#include "geo.h"

namespace geo {

double Circle::radius() const { return 1.0; }

double Circle::diameter() const { return 2.0 * radius(); }

double area(const Circle& circle) {
  return circle.radius() * circle.radius() * 3.14;
}

double perimeter(const Circle& circle) { return 2.0 * 3.14 * circle.radius(); }

}  // namespace geo
