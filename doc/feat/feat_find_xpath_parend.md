When finding the element. 
Add a floating div with the xpath of the parent element on top of the element.

This xpath of the element must be clickable and it will navigate to the parent element.

Example:

```xml
<root>
    <parent>
        <child>
            <grandchild>
                <target/>
            </grandchild>
        </child>
    </parent>
</root>
```

When the user finds the element "target", the floating div will show the xpath of the parent element "grandchild".

```
/root/parent/child/grandchild
```

If the user clicks on the floating div, it will navigate to the parent element "grandchild" and highlight it.