#include "geo.h"

namespace geo {

double report(const Circle& circle) { return area(circle) + circle.radius(); }

}  // namespace geo
